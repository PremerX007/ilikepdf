use std::fs;
use std::io::Cursor;
use std::path::Path;

use ilikepdf_pdf::PdfRenderRequest;

use super::{
    RewriteStructuralPdfRequest, StructuralPdfEngine, StructuralPdfEngineInfo, StructuralPdfError,
    StructuralPdfRewriteResult, StructuralPdfValidation,
};
use crate::application::output::PendingExactPdfOutput;
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
    let source_page_count = verifier.verify(&request.source_path)?;
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
    let output_page_count = verifier.verify(pending.working_path()).map_err(|_| {
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
    fn verify(&self, path: &Path) -> ApplicationResult<u32>;
}

struct NativePdfVerifier;

impl PdfVerifier for NativePdfVerifier {
    fn verify(&self, path: &Path) -> ApplicationResult<u32> {
        let info = ilikepdf_pdf::inspect_document(path)?;
        if info.page_count > 0 {
            let mut rendered = Cursor::new(Vec::new());
            ilikepdf_pdf::render_page_to_png(
                PdfRenderRequest {
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

#[cfg(test)]
mod tests;
