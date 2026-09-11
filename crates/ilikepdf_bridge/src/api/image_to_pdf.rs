use std::path::PathBuf;

use crate::frb_generated::StreamSink;

use super::application::ApplicationError;

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
            page_size: match request.page_size {
                ImagePdfPageSize::Fit => ilikepdf_core::ImagePdfPageSize::Fit,
                ImagePdfPageSize::A4 => ilikepdf_core::ImagePdfPageSize::A4,
                ImagePdfPageSize::UsLetter => ilikepdf_core::ImagePdfPageSize::UsLetter,
            },
            orientation: match request.orientation {
                ImagePdfOrientation::Portrait => ilikepdf_core::ImagePdfOrientation::Portrait,
                ImagePdfOrientation::Landscape => ilikepdf_core::ImagePdfOrientation::Landscape,
            },
            margin: match request.margin {
                ImagePdfMargin::None => ilikepdf_core::ImagePdfMargin::None,
                ImagePdfMargin::Small => ilikepdf_core::ImagePdfMargin::Small,
                ImagePdfMargin::Big => ilikepdf_core::ImagePdfMargin::Big,
            },
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

fn paths_to_strings(paths: Vec<PathBuf>) -> Vec<String> {
    paths
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}
