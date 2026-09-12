use std::path::{Path, PathBuf};

use super::backend::{NativePdfBackend, PdfBackend};
use super::export::export_inspected_pdf_to_images_with_backend;
use crate::application::output::validate_writable_output_directory;
use crate::{ApplicationError, ApplicationErrorCode};

use super::model::{
    ExportPdfBatchRequest, ExportPdfToImagesRequest, PdfBatchDestinationMode,
    PdfBatchDocumentResult, PdfBatchFailure, PdfBatchProgress, PdfBatchResult,
};

pub fn export_pdf_batch_to_images(
    request: ExportPdfBatchRequest,
    on_progress: impl FnMut(PdfBatchProgress),
) -> Result<PdfBatchResult, PdfBatchFailure> {
    export_pdf_batch_to_images_with_backend(&NativePdfBackend, request, on_progress)
}

enum PlannedDocument {
    Ready {
        source_path: PathBuf,
        display_name: String,
        page_count: u32,
    },
    Failed(PdfBatchDocumentResult),
}

fn export_pdf_batch_to_images_with_backend(
    backend: &impl PdfBackend,
    request: ExportPdfBatchRequest,
    mut on_progress: impl FnMut(PdfBatchProgress),
) -> Result<PdfBatchResult, PdfBatchFailure> {
    let total_document_count = u32::try_from(request.source_paths.len()).map_err(|_| {
        PdfBatchFailure::before_start(
            0,
            ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "Too many PDF documents were selected",
            ),
        )
    })?;
    if total_document_count == 0 {
        return Err(PdfBatchFailure::before_start(
            0,
            ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "Select at least one PDF document",
            ),
        ));
    }

    let custom_destination = match request.destination_mode {
        PdfBatchDestinationMode::NextToSourceFiles => None,
        PdfBatchDestinationMode::CustomFolder => {
            let directory = request.custom_destination_directory.ok_or_else(|| {
                PdfBatchFailure::before_start(
                    total_document_count,
                    ApplicationError::new(
                        ApplicationErrorCode::InvalidOutputDirectory,
                        "Choose an existing output directory",
                    ),
                )
            })?;
            validate_writable_output_directory(&directory)
                .map_err(|error| PdfBatchFailure::before_start(total_document_count, error))?;
            Some(directory)
        }
    };

    let mut planned = Vec::with_capacity(request.source_paths.len());
    let mut total_page_count = 0_u32;
    for (index, source_path) in request.source_paths.into_iter().enumerate() {
        let display_name = display_name(&source_path);
        let inspected = if has_pdf_extension(&source_path) {
            backend.inspect_document(&source_path)
        } else {
            Err(ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "Only PDF documents are supported",
            ))
        };
        match inspected {
            Ok(info) => {
                total_page_count =
                    total_page_count
                        .checked_add(info.page_count)
                        .ok_or_else(|| {
                            PdfBatchFailure::during_batch(
                                total_document_count,
                                0,
                                Some(index_u32(index)),
                                Some(display_name.clone()),
                                total_page_count,
                                0,
                                Vec::new(),
                                ApplicationError::new(
                                    ApplicationErrorCode::InvalidRequest,
                                    "The batch contains too many PDF pages",
                                ),
                            )
                        })?;
                planned.push(PlannedDocument::Ready {
                    source_path,
                    display_name,
                    page_count: info.page_count,
                });
            }
            Err(error) if is_global_error(error.code) => {
                return Err(PdfBatchFailure::during_batch(
                    total_document_count,
                    0,
                    Some(index_u32(index)),
                    Some(display_name),
                    total_page_count,
                    0,
                    Vec::new(),
                    error,
                ));
            }
            Err(error) => planned.push(PlannedDocument::Failed(PdfBatchDocumentResult {
                source_path,
                display_name,
                total_page_count: 0,
                completed_page_count: 0,
                output_files: Vec::new(),
                error: Some(error),
            })),
        }
    }

    let mut completed_document_count = 0_u32;
    let mut completed_page_count = 0_u32;
    let mut documents = Vec::with_capacity(planned.len());

    for (index, document) in planned.into_iter().enumerate() {
        match document {
            PlannedDocument::Failed(result) => {
                let filename = result.display_name.clone();
                documents.push(result);
                completed_document_count += 1;
                on_progress(PdfBatchProgress {
                    total_document_count,
                    completed_document_count,
                    current_document_index: Some(index_u32(index)),
                    current_document_filename: Some(filename),
                    total_page_count,
                    completed_page_count,
                    current_page: None,
                });
            }
            PlannedDocument::Ready {
                source_path,
                display_name,
                page_count,
            } => {
                let destination_directory = match &custom_destination {
                    Some(directory) => directory.clone(),
                    None => source_directory(&source_path).map_err(|error| {
                        PdfBatchFailure::during_batch(
                            total_document_count,
                            completed_document_count,
                            Some(index_u32(index)),
                            Some(display_name.clone()),
                            total_page_count,
                            completed_page_count,
                            documents.clone(),
                            error,
                        )
                    })?,
                };
                on_progress(PdfBatchProgress {
                    total_document_count,
                    completed_document_count,
                    current_document_index: Some(index_u32(index)),
                    current_document_filename: Some(display_name.clone()),
                    total_page_count,
                    completed_page_count,
                    current_page: (page_count > 0).then_some(1),
                });

                let exported = export_inspected_pdf_to_images_with_backend(
                    backend,
                    ExportPdfToImagesRequest {
                        source_path: source_path.clone(),
                        destination_directory,
                        quality: request.quality,
                        format: request.format,
                    },
                    page_count,
                    |progress| {
                        on_progress(PdfBatchProgress {
                            total_document_count,
                            completed_document_count,
                            current_document_index: Some(index_u32(index)),
                            current_document_filename: Some(display_name.clone()),
                            total_page_count,
                            completed_page_count: completed_page_count
                                + progress.completed_page_count,
                            current_page: progress.current_page,
                        });
                    },
                );

                match exported {
                    Ok(result) => {
                        completed_page_count += result.completed_page_count;
                        documents.push(PdfBatchDocumentResult {
                            source_path,
                            display_name: display_name.clone(),
                            total_page_count: result.total_page_count,
                            completed_page_count: result.completed_page_count,
                            output_files: result.output_files,
                            error: None,
                        });
                    }
                    Err(failure) => {
                        completed_page_count += failure.completed_page_count;
                        let error = failure.error;
                        documents.push(PdfBatchDocumentResult {
                            source_path,
                            display_name: display_name.clone(),
                            total_page_count: failure.total_page_count,
                            completed_page_count: failure.completed_page_count,
                            output_files: failure.output_files,
                            error: Some(error.clone()),
                        });
                        if is_global_error(error.code) {
                            completed_document_count += 1;
                            return Err(PdfBatchFailure::during_batch(
                                total_document_count,
                                completed_document_count,
                                Some(index_u32(index)),
                                Some(display_name),
                                total_page_count,
                                completed_page_count,
                                documents,
                                error,
                            ));
                        }
                    }
                }
                completed_document_count += 1;
                on_progress(PdfBatchProgress {
                    total_document_count,
                    completed_document_count,
                    current_document_index: Some(index_u32(index)),
                    current_document_filename: Some(display_name),
                    total_page_count,
                    completed_page_count,
                    current_page: None,
                });
            }
        }
    }

    let succeeded_document_count = u32::try_from(
        documents
            .iter()
            .filter(|document| document.error.is_none())
            .count(),
    )
    .expect("document count was represented by u32");
    let failed_document_count = total_document_count - succeeded_document_count;

    Ok(PdfBatchResult {
        total_document_count,
        succeeded_document_count,
        failed_document_count,
        total_page_count,
        completed_page_count,
        documents,
    })
}

impl PdfBatchFailure {
    fn before_start(total_document_count: u32, error: ApplicationError) -> Self {
        Self::during_batch(total_document_count, 0, None, None, 0, 0, Vec::new(), error)
    }

    #[allow(clippy::too_many_arguments)]
    fn during_batch(
        total_document_count: u32,
        completed_document_count: u32,
        current_document_index: Option<u32>,
        current_document_filename: Option<String>,
        total_page_count: u32,
        completed_page_count: u32,
        documents: Vec<PdfBatchDocumentResult>,
        error: ApplicationError,
    ) -> Self {
        Self {
            total_document_count,
            completed_document_count,
            current_document_index,
            current_document_filename,
            total_page_count,
            completed_page_count,
            documents,
            error,
        }
    }
}

fn source_directory(source_path: &Path) -> Result<PathBuf, ApplicationError> {
    match source_path
        .parent()
        .filter(|directory| !directory.as_os_str().is_empty())
    {
        Some(directory) => Ok(directory.to_path_buf()),
        None => std::env::current_dir().map_err(|_| {
            ApplicationError::new(
                ApplicationErrorCode::InvalidOutputDirectory,
                "The source PDF directory could not be resolved",
            )
        }),
    }
}

fn display_name(source_path: &Path) -> String {
    source_path
        .file_name()
        .unwrap_or(source_path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn has_pdf_extension(source_path: &Path) -> bool {
    source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
}

fn index_u32(index: usize) -> u32 {
    u32::try_from(index + 1).expect("document count was represented by u32")
}

fn is_global_error(code: ApplicationErrorCode) -> bool {
    code == ApplicationErrorCode::PdfRuntimeUnavailable
}

#[cfg(test)]
mod tests;
