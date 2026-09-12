use std::path::PathBuf;

use crate::frb_generated::StreamSink;

use super::application::ApplicationError;
use super::mapping::paths_to_strings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePdfPageSize {
    Fit,
    A4,
    UsLetter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePdfOrientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePdfMargin {
    None,
    Small,
    Big,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateImagePdfRequest {
    pub source_paths: Vec<String>,
    pub destination_directory: String,
    pub page_size: ImagePdfPageSize,
    pub orientation: ImagePdfOrientation,
    pub margin: ImagePdfMargin,
    pub merge: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePdfStatus {
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePdfUpdate {
    pub status: ImagePdfStatus,
    pub total_image_count: u32,
    pub completed_image_count: u32,
    pub current_image: Option<u32>,
    pub output_files: Vec<String>,
    pub error: Option<ApplicationError>,
}

/// Creates PDFs sequentially on flutter_rust_bridge's worker pool and streams progress.
pub fn create_pdfs_from_images(
    request: CreateImagePdfRequest,
    progress_sink: StreamSink<ImagePdfUpdate>,
) {
    crate::logging::record(crate::logging::Event::ImagePdfCreationRequested);

    let result = ilikepdf_core::create_pdfs_from_images(
        ilikepdf_core::CreateImagePdfRequest {
            source_paths: request
                .source_paths
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            destination_directory: PathBuf::from(request.destination_directory),
            page_size: request.page_size.into(),
            orientation: request.orientation.into(),
            margin: request.margin.into(),
            merge: request.merge,
        },
        |progress| {
            let _ = progress_sink.add(ImagePdfUpdate {
                status: ImagePdfStatus::Running,
                total_image_count: progress.total_image_count,
                completed_image_count: progress.completed_image_count,
                current_image: progress.current_image,
                output_files: Vec::new(),
                error: None,
            });
        },
    );

    let update = match result {
        Ok(result) => ImagePdfUpdate {
            status: ImagePdfStatus::Complete,
            total_image_count: result.total_image_count,
            completed_image_count: result.completed_image_count,
            current_image: None,
            output_files: paths_to_strings(result.output_files),
            error: None,
        },
        Err(failure) => ImagePdfUpdate {
            status: ImagePdfStatus::Failed,
            total_image_count: failure.total_image_count,
            completed_image_count: failure.completed_image_count,
            current_image: failure.current_image,
            output_files: paths_to_strings(failure.output_files),
            error: Some(failure.error.into()),
        },
    };
    let _ = progress_sink.add(update);
}
