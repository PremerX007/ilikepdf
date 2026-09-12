use std::path::{Path, PathBuf};

use super::image_output::validate_writable_output_directory;
use super::pdf_export::{
    ExportPdfToImagesRequest, NativePdfBackend, PdfBackend, PdfExportFormat, PdfExportQuality,
    export_inspected_pdf_to_images_with_backend,
};
use crate::{ApplicationError, ApplicationErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfBatchDestinationMode {
    NextToSourceFiles,
    CustomFolder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPdfBatchRequest {
    pub source_paths: Vec<PathBuf>,
    pub destination_mode: PdfBatchDestinationMode,
    pub custom_destination_directory: Option<PathBuf>,
    pub quality: PdfExportQuality,
    pub format: PdfExportFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfBatchProgress {
    pub total_document_count: u32,
    pub completed_document_count: u32,
    pub current_document_index: Option<u32>,
    pub current_document_filename: Option<String>,
    pub total_page_count: u32,
    pub completed_page_count: u32,
    pub current_page: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfBatchDocumentResult {
    pub source_path: PathBuf,
    pub display_name: String,
    pub total_page_count: u32,
    pub completed_page_count: u32,
    pub output_files: Vec<PathBuf>,
    pub error: Option<ApplicationError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfBatchResult {
    pub total_document_count: u32,
    pub succeeded_document_count: u32,
    pub failed_document_count: u32,
    pub total_page_count: u32,
    pub completed_page_count: u32,
    pub documents: Vec<PdfBatchDocumentResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfBatchFailure {
    pub total_document_count: u32,
    pub completed_document_count: u32,
    pub current_document_index: Option<u32>,
    pub current_document_filename: Option<String>,
    pub total_page_count: u32,
    pub completed_page_count: u32,
    pub documents: Vec<PdfBatchDocumentResult>,
    pub error: ApplicationError,
}

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
mod tests {
    use std::fs;

    use ilikepdf_pdf::PdfRenderer;

    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("ilikepdf_pdf")
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn renderer() -> &'static PdfRenderer {
        crate::test_pdf_renderer()
    }

    #[test]
    fn mixed_batch_continues_in_visual_order_and_reports_overall_progress() {
        let sources = tempfile::tempdir().expect("source directory should be created");
        let destination = tempfile::tempdir().expect("destination should be created");
        let cover = sources.path().join("cover.pdf");
        let broken = sources.path().join("broken.pdf");
        let report = sources.path().join("report.pdf");
        fs::copy(fixture("one_page.pdf"), &cover).expect("cover should be copied");
        fs::copy(fixture("malformed.pdf"), &broken).expect("broken PDF should be copied");
        fs::copy(fixture("two_page.pdf"), &report).expect("report should be copied");
        let source_bytes = [
            fs::read(&cover).expect("cover should be readable"),
            fs::read(&broken).expect("broken PDF should be readable"),
            fs::read(&report).expect("report should be readable"),
        ];
        let mut progress = Vec::new();

        let result = export_pdf_batch_to_images_with_backend(
            renderer(),
            ExportPdfBatchRequest {
                source_paths: vec![cover.clone(), broken.clone(), report.clone()],
                destination_mode: PdfBatchDestinationMode::CustomFolder,
                custom_destination_directory: Some(destination.path().to_path_buf()),
                quality: PdfExportQuality::Standard,
                format: PdfExportFormat::Png,
            },
            |update| progress.push(update),
        )
        .expect("one malformed PDF should not stop later documents");

        assert_eq!(result.total_document_count, 3);
        assert_eq!(result.succeeded_document_count, 2);
        assert_eq!(result.failed_document_count, 1);
        assert_eq!(result.total_page_count, 3);
        assert_eq!(result.completed_page_count, 3);
        assert_eq!(
            result
                .documents
                .iter()
                .map(|document| document.display_name.as_str())
                .collect::<Vec<_>>(),
            ["cover.pdf", "broken.pdf", "report.pdf"]
        );
        assert_eq!(
            result.documents[1]
                .error
                .as_ref()
                .expect("broken PDF should fail")
                .code,
            ApplicationErrorCode::InvalidPdf
        );
        assert_eq!(
            result.documents[0].output_files,
            [destination.path().join("cover-page-0001.png")]
        );
        assert_eq!(
            result.documents[2].output_files[0].parent(),
            Some(destination.path().join("report").as_path())
        );
        assert_eq!(
            progress
                .last()
                .expect("batch should report final progress")
                .completed_page_count,
            3
        );
        assert_eq!(fs::read(cover).unwrap(), source_bytes[0]);
        assert_eq!(fs::read(broken).unwrap(), source_bytes[1]);
        assert_eq!(fs::read(report).unwrap(), source_bytes[2]);
    }

    #[test]
    fn next_to_source_files_uses_each_documents_own_directory() {
        let first_directory = tempfile::tempdir().expect("first directory should be created");
        let second_directory = tempfile::tempdir().expect("second directory should be created");
        let cover = first_directory.path().join("cover.pdf");
        let report = second_directory.path().join("report.pdf");
        fs::copy(fixture("one_page.pdf"), &cover).expect("cover should be copied");
        fs::copy(fixture("two_page.pdf"), &report).expect("report should be copied");

        let result = export_pdf_batch_to_images_with_backend(
            renderer(),
            ExportPdfBatchRequest {
                source_paths: vec![cover, report],
                destination_mode: PdfBatchDestinationMode::NextToSourceFiles,
                custom_destination_directory: None,
                quality: PdfExportQuality::Standard,
                format: PdfExportFormat::Png,
            },
            |_| {},
        )
        .expect("both PDFs should export beside themselves");

        assert_eq!(
            result.documents[0].output_files,
            [first_directory.path().join("cover-page-0001.png")]
        );
        assert_eq!(
            result.documents[1].output_files[0].parent(),
            Some(second_directory.path().join("report").as_path())
        );
    }

    #[test]
    fn invalid_custom_destination_stops_before_processing_documents() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let invalid_destination = directory.path().join("not-a-directory");
        fs::write(&invalid_destination, b"occupied")
            .expect("invalid destination fixture should be created");

        let failure = export_pdf_batch_to_images_with_backend(
            renderer(),
            ExportPdfBatchRequest {
                source_paths: vec![fixture("one_page.pdf")],
                destination_mode: PdfBatchDestinationMode::CustomFolder,
                custom_destination_directory: Some(invalid_destination),
                quality: PdfExportQuality::Standard,
                format: PdfExportFormat::Png,
            },
            |_| panic!("global validation failure must not report document progress"),
        )
        .expect_err("invalid custom destination should stop the batch");

        assert_eq!(failure.completed_document_count, 0);
        assert!(failure.documents.is_empty());
        assert_eq!(
            failure.error.code,
            ApplicationErrorCode::InvalidOutputDirectory
        );
    }

    #[test]
    fn repeated_batch_uses_numbered_files_and_folders_without_clobbering() {
        let sources = tempfile::tempdir().expect("source directory should be created");
        let destination = tempfile::tempdir().expect("destination should be created");
        let cover = sources.path().join("cover.pdf");
        let report = sources.path().join("report.pdf");
        fs::copy(fixture("one_page.pdf"), &cover).expect("cover should be copied");
        fs::copy(fixture("two_page.pdf"), &report).expect("report should be copied");
        let request = || ExportPdfBatchRequest {
            source_paths: vec![cover.clone(), report.clone()],
            destination_mode: PdfBatchDestinationMode::CustomFolder,
            custom_destination_directory: Some(destination.path().to_path_buf()),
            quality: PdfExportQuality::Standard,
            format: PdfExportFormat::Png,
        };

        let first = export_pdf_batch_to_images_with_backend(renderer(), request(), |_| {})
            .expect("first batch should export");
        let first_png_bytes = fs::read(&first.documents[0].output_files[0]).unwrap();
        let second = export_pdf_batch_to_images_with_backend(renderer(), request(), |_| {})
            .expect("second batch should be numbered");
        let third = export_pdf_batch_to_images_with_backend(renderer(), request(), |_| {})
            .expect("third batch should be numbered");

        assert_eq!(
            second.documents[0].output_files,
            [destination.path().join("cover-page-0001 (1).png")]
        );
        assert_eq!(
            third.documents[0].output_files,
            [destination.path().join("cover-page-0001 (2).png")]
        );
        assert_eq!(
            second.documents[1].output_files[0].parent(),
            Some(destination.path().join("report (1)").as_path())
        );
        assert_eq!(
            third.documents[1].output_files[0].parent(),
            Some(destination.path().join("report (2)").as_path())
        );
        assert_eq!(
            second.documents[1].output_files[0].file_name(),
            Some(std::ffi::OsStr::new("report-page-0001.png"))
        );
        assert_eq!(
            fs::read(&first.documents[0].output_files[0]).unwrap(),
            first_png_bytes
        );
    }
}
