use std::fs;
use std::path::Path;

use super::{
    MergePdfFailure, MergePdfProgress, MergePdfRequest, MergePdfResult, MergePdfStage,
    RewriteStructuralPdfRequest, StructuralPdfEngine, StructuralPdfEngineInfo, StructuralPdfError,
    StructuralPdfMergeRequest, StructuralPdfRewriteResult, StructuralPdfValidation,
};
use crate::application::output::{PendingExactPdfOutput, PendingNumberedPdfOutput};
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

pub fn probe_structural_pdf_engine(
    engine: &dyn StructuralPdfEngine,
) -> ApplicationResult<StructuralPdfEngineInfo> {
    engine.probe().map_err(map_engine_error)
}

pub fn validate_structural_pdf(
    engine: &dyn StructuralPdfEngine,
    source_path: &Path,
) -> ApplicationResult<StructuralPdfValidation> {
    validate_source(source_path)?;
    engine.validate(source_path).map_err(map_engine_error)
}

pub fn rewrite_structural_pdf(
    engine: &dyn StructuralPdfEngine,
    request: RewriteStructuralPdfRequest,
) -> ApplicationResult<StructuralPdfRewriteResult> {
    rewrite_structural_pdf_with_verifier(engine, &NativePdfVerifier, request)
}

pub fn merge_pdf(
    engine: &dyn StructuralPdfEngine,
    request: MergePdfRequest,
    on_progress: impl FnMut(MergePdfProgress),
) -> Result<MergePdfResult, MergePdfFailure> {
    merge_pdf_with_verifier(engine, &NativePdfVerifier, request, on_progress)
}

fn merge_pdf_with_verifier(
    engine: &dyn StructuralPdfEngine,
    verifier: &impl PdfVerifier,
    request: MergePdfRequest,
    mut on_progress: impl FnMut(MergePdfProgress),
) -> Result<MergePdfResult, MergePdfFailure> {
    let input_count = u32::try_from(request.source_paths.len()).map_err(|_| {
        MergePdfFailure::global(
            ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "Too many PDF inputs were selected",
            ),
            u32::MAX,
            0,
        )
    })?;
    on_progress(MergePdfProgress {
        stage: MergePdfStage::Preparing,
        input_count,
        total_page_count: 0,
    });
    if input_count < 2 {
        return Err(MergePdfFailure::global(
            ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "Select at least two PDFs to merge",
            ),
            input_count,
            0,
        ));
    }

    let pending = PendingNumberedPdfOutput::in_directory(
        &request.destination_directory,
        &request.output_name,
    )
    .map_err(|error| MergePdfFailure::global(error, input_count, 0))?;
    let mut expected_page_count = 0_u32;
    let mut warning_input_count = 0_u32;
    for (index, source_path) in request.source_paths.iter().enumerate() {
        validate_source(source_path).map_err(|error| {
            MergePdfFailure::for_input(error, index, source_path, input_count, expected_page_count)
        })?;
        let validation = engine.validate(source_path).map_err(|error| {
            MergePdfFailure::for_input(
                map_engine_error(error),
                index,
                source_path,
                input_count,
                expected_page_count,
            )
        })?;
        warning_input_count += u32::from(validation.has_warnings);
        let page_count = verifier.verify(source_path, false).map_err(|error| {
            MergePdfFailure::for_input(error, index, source_path, input_count, expected_page_count)
        })?;
        expected_page_count = expected_page_count.checked_add(page_count).ok_or_else(|| {
            MergePdfFailure::for_input(
                ApplicationError::new(
                    ApplicationErrorCode::InvalidRequest,
                    "The merged PDF page count is too large",
                ),
                index,
                source_path,
                input_count,
                expected_page_count,
            )
        })?;
    }

    on_progress(MergePdfProgress {
        stage: MergePdfStage::Merging,
        input_count,
        total_page_count: expected_page_count,
    });
    let operation = engine
        .merge(&StructuralPdfMergeRequest {
            ordered_source_paths: request.source_paths,
            working_output_path: pending.working_path().to_path_buf(),
        })
        .map_err(|error| {
            MergePdfFailure::global(map_engine_error(error), input_count, expected_page_count)
        })?;

    on_progress(MergePdfProgress {
        stage: MergePdfStage::Validating,
        input_count,
        total_page_count: expected_page_count,
    });
    let output_metadata = fs::metadata(pending.working_path()).map_err(|_| {
        MergePdfFailure::global(
            output_validation_error("The merged PDF could not be validated"),
            input_count,
            expected_page_count,
        )
    })?;
    if !output_metadata.is_file() || output_metadata.len() == 0 {
        return Err(MergePdfFailure::global(
            output_validation_error("The merged PDF could not be validated"),
            input_count,
            expected_page_count,
        ));
    }
    let output_validation = engine.validate(pending.working_path()).map_err(|_| {
        MergePdfFailure::global(
            output_validation_error("The merged PDF failed structural validation"),
            input_count,
            expected_page_count,
        )
    })?;
    let output_page_count = verifier
        .verify(pending.working_path(), false)
        .map_err(|_| {
            MergePdfFailure::global(
                output_validation_error("The merged PDF could not be reopened"),
                input_count,
                expected_page_count,
            )
        })?;
    if output_page_count != expected_page_count {
        return Err(MergePdfFailure::global(
            output_validation_error("The merged PDF page count was not as expected"),
            input_count,
            expected_page_count,
        ));
    }

    on_progress(MergePdfProgress {
        stage: MergePdfStage::Publishing,
        input_count,
        total_page_count: expected_page_count,
    });
    let output_path = pending
        .publish()
        .map_err(|error| MergePdfFailure::global(error, input_count, expected_page_count))?;
    on_progress(MergePdfProgress {
        stage: MergePdfStage::Completed,
        input_count,
        total_page_count: expected_page_count,
    });

    Ok(MergePdfResult {
        output_path,
        input_count,
        page_count: output_page_count,
        warning_input_count,
        has_warnings: warning_input_count > 0
            || operation.has_warnings
            || output_validation.has_warnings,
    })
}

fn rewrite_structural_pdf_with_verifier(
    engine: &dyn StructuralPdfEngine,
    verifier: &impl PdfVerifier,
    request: RewriteStructuralPdfRequest,
) -> ApplicationResult<StructuralPdfRewriteResult> {
    validate_source(&request.source_path)?;
    let pending = PendingExactPdfOutput::for_destination(&request.destination_path)?;
    let validation = engine
        .validate(&request.source_path)
        .map_err(map_engine_error)?;
    let source_page_count = verifier.verify(&request.source_path, true)?;
    let operation = engine
        .rewrite(&request.source_path, pending.working_path())
        .map_err(map_engine_error)?;
    let output_metadata = fs::metadata(pending.working_path()).map_err(|_| {
        ApplicationError::new(
            ApplicationErrorCode::StructuralPdfOutputValidationFailed,
            "The rewritten PDF could not be validated",
        )
    })?;
    if !output_metadata.is_file() || output_metadata.len() == 0 {
        return Err(ApplicationError::new(
            ApplicationErrorCode::StructuralPdfOutputValidationFailed,
            "The rewritten PDF could not be validated",
        ));
    }
    let output_page_count = verifier.verify(pending.working_path(), true).map_err(|_| {
        ApplicationError::new(
            ApplicationErrorCode::StructuralPdfOutputValidationFailed,
            "The rewritten PDF could not be validated",
        )
    })?;
    if output_page_count != source_page_count {
        return Err(ApplicationError::new(
            ApplicationErrorCode::StructuralPdfOutputValidationFailed,
            "The rewritten PDF page count did not match the source",
        ));
    }
    let output_path = pending.publish()?;

    Ok(StructuralPdfRewriteResult {
        output_path,
        page_count: output_page_count,
        has_warnings: validation.has_warnings || operation.has_warnings,
    })
}

trait PdfVerifier {
    fn verify(&self, path: &Path, render_representative_page: bool) -> ApplicationResult<u32>;
}

struct NativePdfVerifier;

impl PdfVerifier for NativePdfVerifier {
    fn verify(&self, path: &Path, render_representative_page: bool) -> ApplicationResult<u32> {
        let info = ilikepdf_pdf::inspect_document(path)?;
        if render_representative_page && info.page_count > 0 {
            let mut rendered = std::io::Cursor::new(Vec::new());
            ilikepdf_pdf::render_page_to_png(
                ilikepdf_pdf::PdfRenderRequest {
                    source_path: path.to_path_buf(),
                    page_index: 0,
                    target_width: 64,
                },
                &mut rendered,
            )?;
        }
        Ok(info.page_count)
    }
}

fn validate_source(source_path: &Path) -> ApplicationResult<()> {
    match fs::metadata(source_path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(ApplicationError::new(
            ApplicationErrorCode::SourceNotFile,
            "The selected path is not a file",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(ApplicationError::new(
            ApplicationErrorCode::SourceNotFound,
            "The selected PDF no longer exists",
        )),
        Err(_) => Err(ApplicationError::new(
            ApplicationErrorCode::SourceUnreadable,
            "The selected PDF could not be read",
        )),
    }
}

fn map_engine_error(error: StructuralPdfError) -> ApplicationError {
    let (code, message) = match error {
        StructuralPdfError::RuntimeUnavailable => (
            ApplicationErrorCode::StructuralPdfRuntimeUnavailable,
            "The local structural PDF runtime is unavailable",
        ),
        StructuralPdfError::RuntimeIncompatible => (
            ApplicationErrorCode::StructuralPdfRuntimeIncompatible,
            "The local structural PDF runtime is incompatible",
        ),
        StructuralPdfError::RuntimeLaunchFailed => (
            ApplicationErrorCode::StructuralPdfLaunchFailed,
            "The local structural PDF runtime could not be started",
        ),
        StructuralPdfError::SourceNotFound => (
            ApplicationErrorCode::SourceNotFound,
            "The selected PDF no longer exists",
        ),
        StructuralPdfError::SourceNotFile => (
            ApplicationErrorCode::SourceNotFile,
            "The selected path is not a file",
        ),
        StructuralPdfError::SourceUnreadable => (
            ApplicationErrorCode::SourceUnreadable,
            "The selected PDF could not be read",
        ),
        StructuralPdfError::PasswordRequired => (
            ApplicationErrorCode::PasswordRequired,
            "This PDF is password protected. Unlock it before merging",
        ),
        StructuralPdfError::InvalidDocument => (
            ApplicationErrorCode::InvalidPdf,
            "The selected file is not a structurally valid PDF",
        ),
        StructuralPdfError::OutputWriteFailed => (
            ApplicationErrorCode::OutputWriteFailed,
            "The rewritten PDF could not be written",
        ),
        StructuralPdfError::OperationFailed => (
            ApplicationErrorCode::StructuralPdfOperationFailed,
            "The structural PDF operation failed",
        ),
    };
    ApplicationError::new(code, message)
}

fn output_validation_error(message: &'static str) -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::StructuralPdfOutputValidationFailed,
        message,
    )
}

impl MergePdfFailure {
    fn global(error: ApplicationError, input_count: u32, expected_page_count: u32) -> Self {
        Self {
            error,
            input_index: None,
            input_path: None,
            input_count,
            expected_page_count,
        }
    }

    fn for_input(
        error: ApplicationError,
        input_index: usize,
        input_path: &Path,
        input_count: u32,
        expected_page_count: u32,
    ) -> Self {
        Self {
            error,
            input_index: u32::try_from(input_index).ok(),
            input_path: Some(input_path.to_path_buf()),
            input_count,
            expected_page_count,
        }
    }
}

#[cfg(test)]
mod tests;
