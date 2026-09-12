//! Image-to-PDF application contracts and workflow façade.

use std::path::PathBuf;

use crate::ApplicationError;

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
    pub source_paths: Vec<PathBuf>,
    pub destination_directory: PathBuf,
    pub page_size: ImagePdfPageSize,
    pub orientation: ImagePdfOrientation,
    pub margin: ImagePdfMargin,
    pub merge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePdfProgress {
    pub total_image_count: u32,
    pub completed_image_count: u32,
    pub current_image: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePdfResult {
    pub total_image_count: u32,
    pub completed_image_count: u32,
    pub output_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePdfFailure {
    pub total_image_count: u32,
    pub completed_image_count: u32,
    pub current_image: Option<u32>,
    pub output_files: Vec<PathBuf>,
    pub error: ApplicationError,
}

mod backend;
mod planning;
mod workflow;

pub use workflow::create_pdfs_from_images;
