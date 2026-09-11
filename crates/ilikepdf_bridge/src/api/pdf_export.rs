use std::path::PathBuf;

use crate::frb_generated::StreamSink;

use super::application::ApplicationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfExportQuality {
    Standard,
    HighQuality,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPdfToImagesRequest {
    pub source_path: String,
    pub destination_directory: String,
    pub quality: PdfExportQuality,
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
            quality: match request.quality {
                PdfExportQuality::Standard => ilikepdf_core::PdfExportQuality::Standard,
                PdfExportQuality::HighQuality => ilikepdf_core::PdfExportQuality::HighQuality,
            },
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

fn paths_to_strings(paths: Vec<PathBuf>) -> Vec<String> {
    paths
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}
