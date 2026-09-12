use std::path::PathBuf;

use crate::frb_generated::StreamSink;

use super::application::ApplicationError;
use super::mapping::paths_to_strings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfExportQuality {
    Standard,
    HighQuality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfExportFormat {
    Png,
    Jpg,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPdfToImagesRequest {
    pub source_path: String,
    pub destination_directory: String,
    pub quality: PdfExportQuality,
    pub format: PdfExportFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfExportStatus {
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfExportUpdate {
    pub status: PdfExportStatus,
    pub total_page_count: u32,
    pub completed_page_count: u32,
    pub current_page: Option<u32>,
    pub output_files: Vec<String>,
    pub error: Option<ApplicationError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfBatchDestinationMode {
    NextToSourceFiles,
    CustomFolder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPdfBatchRequest {
    pub source_paths: Vec<String>,
    pub destination_mode: PdfBatchDestinationMode,
    pub custom_destination_directory: Option<String>,
    pub quality: PdfExportQuality,
    pub format: PdfExportFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfBatchExportStatus {
    Running,
    Complete,
    CompleteWithErrors,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfBatchDocumentResult {
    pub source_path: String,
    pub display_name: String,
    pub total_page_count: u32,
    pub completed_page_count: u32,
    pub output_files: Vec<String>,
    pub error: Option<ApplicationError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfBatchExportUpdate {
    pub status: PdfBatchExportStatus,
    pub total_document_count: u32,
    pub completed_document_count: u32,
    pub succeeded_document_count: u32,
    pub failed_document_count: u32,
    pub current_document_index: Option<u32>,
    pub current_document_filename: Option<String>,
    pub total_page_count: u32,
    pub completed_page_count: u32,
    pub current_page: Option<u32>,
    pub documents: Vec<PdfBatchDocumentResult>,
    pub error: Option<ApplicationError>,
}

/// Exports every page sequentially on flutter_rust_bridge's worker pool and streams progress.
pub fn export_pdf_to_images(
    request: ExportPdfToImagesRequest,
    progress_sink: StreamSink<PdfExportUpdate>,
) {
    crate::logging::record(crate::logging::Event::PdfImageExportRequested);

    let result = ilikepdf_core::export_pdf_to_images(
        ilikepdf_core::ExportPdfToImagesRequest {
            source_path: PathBuf::from(request.source_path),
            destination_directory: PathBuf::from(request.destination_directory),
            quality: request.quality.into(),
            format: request.format.into(),
        },
        |progress| {
            let _ = progress_sink.add(PdfExportUpdate {
                status: PdfExportStatus::Running,
                total_page_count: progress.total_page_count,
                completed_page_count: progress.completed_page_count,
                current_page: progress.current_page,
                output_files: Vec::new(),
                error: None,
            });
        },
    );

    let update = match result {
        Ok(result) => PdfExportUpdate {
            status: PdfExportStatus::Complete,
            total_page_count: result.total_page_count,
            completed_page_count: result.completed_page_count,
            current_page: None,
            output_files: paths_to_strings(result.output_files),
            error: None,
        },
        Err(failure) => PdfExportUpdate {
            status: PdfExportStatus::Failed,
            total_page_count: failure.total_page_count,
            completed_page_count: failure.completed_page_count,
            current_page: failure.current_page,
            output_files: paths_to_strings(failure.output_files),
            error: Some(failure.error.into()),
        },
    };
    let _ = progress_sink.add(update);
}

/// Exports PDFs sequentially and isolates failures to one document where possible.
pub fn export_pdf_batch_to_images(
    request: ExportPdfBatchRequest,
    progress_sink: StreamSink<PdfBatchExportUpdate>,
) {
    crate::logging::record(crate::logging::Event::PdfImageExportRequested);

    let result = ilikepdf_core::export_pdf_batch_to_images(
        ilikepdf_core::ExportPdfBatchRequest {
            source_paths: request
                .source_paths
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            destination_mode: request.destination_mode.into(),
            custom_destination_directory: request.custom_destination_directory.map(PathBuf::from),
            quality: request.quality.into(),
            format: request.format.into(),
        },
        |progress| {
            let _ = progress_sink.add(PdfBatchExportUpdate {
                status: PdfBatchExportStatus::Running,
                total_document_count: progress.total_document_count,
                completed_document_count: progress.completed_document_count,
                succeeded_document_count: 0,
                failed_document_count: 0,
                current_document_index: progress.current_document_index,
                current_document_filename: progress.current_document_filename,
                total_page_count: progress.total_page_count,
                completed_page_count: progress.completed_page_count,
                current_page: progress.current_page,
                documents: Vec::new(),
                error: None,
            });
        },
    );

    let update = match result {
        Ok(result) => PdfBatchExportUpdate {
            status: if result.failed_document_count == 0 {
                PdfBatchExportStatus::Complete
            } else {
                PdfBatchExportStatus::CompleteWithErrors
            },
            total_document_count: result.total_document_count,
            completed_document_count: result.total_document_count,
            succeeded_document_count: result.succeeded_document_count,
            failed_document_count: result.failed_document_count,
            current_document_index: None,
            current_document_filename: None,
            total_page_count: result.total_page_count,
            completed_page_count: result.completed_page_count,
            current_page: None,
            documents: map_batch_documents(result.documents),
            error: None,
        },
        Err(failure) => PdfBatchExportUpdate {
            status: PdfBatchExportStatus::Failed,
            total_document_count: failure.total_document_count,
            completed_document_count: failure.completed_document_count,
            succeeded_document_count: u32::try_from(
                failure
                    .documents
                    .iter()
                    .filter(|document| document.error.is_none())
                    .count(),
            )
            .expect("document count was represented by u32"),
            failed_document_count: u32::try_from(
                failure
                    .documents
                    .iter()
                    .filter(|document| document.error.is_some())
                    .count(),
            )
            .expect("document count was represented by u32"),
            current_document_index: failure.current_document_index,
            current_document_filename: failure.current_document_filename,
            total_page_count: failure.total_page_count,
            completed_page_count: failure.completed_page_count,
            current_page: None,
            documents: map_batch_documents(failure.documents),
            error: Some(failure.error.into()),
        },
    };
    let _ = progress_sink.add(update);
}

fn map_batch_documents(
    documents: Vec<ilikepdf_core::PdfBatchDocumentResult>,
) -> Vec<PdfBatchDocumentResult> {
    documents
        .into_iter()
        .map(|document| PdfBatchDocumentResult {
            source_path: document.source_path.to_string_lossy().into_owned(),
            display_name: document.display_name,
            total_page_count: document.total_page_count,
            completed_page_count: document.completed_page_count,
            output_files: paths_to_strings(document.output_files),
            error: document.error.map(Into::into),
        })
        .collect()
}
