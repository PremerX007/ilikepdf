use std::path::PathBuf;

use ilikepdf_pdf::PdfImageFormat;

use crate::ApplicationError;

const STANDARD_DPI: u16 = 150;
const HIGH_QUALITY_DPI: u16 = 300;

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

impl PdfExportFormat {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpg => "jpg",
        }
    }

    pub(super) const fn renderer_format(self) -> PdfImageFormat {
        match self {
            Self::Png => PdfImageFormat::Png,
            Self::Jpg => PdfImageFormat::Jpg,
        }
    }
}

impl PdfExportQuality {
    pub const fn dpi(self) -> u16 {
        match self {
            Self::Standard => STANDARD_DPI,
            Self::HighQuality => HIGH_QUALITY_DPI,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPdfToImagesRequest {
    pub source_path: PathBuf,
    pub destination_directory: PathBuf,
    pub quality: PdfExportQuality,
    pub format: PdfExportFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfExportProgress {
    pub total_page_count: u32,
    pub completed_page_count: u32,
    pub current_page: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfExportResult {
    pub total_page_count: u32,
    pub completed_page_count: u32,
    pub output_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfExportFailure {
    pub total_page_count: u32,
    pub completed_page_count: u32,
    pub current_page: Option<u32>,
    pub output_files: Vec<PathBuf>,
    pub error: ApplicationError,
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct PdfPageSize {
    pub width_points: f64,
    pub height_points: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfDocumentInfo {
    pub page_count: u32,
    pub first_page_size: Option<PdfPageSize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderPdfPageRequest {
    pub source_path: PathBuf,
    pub page_index: u32,
    pub target_width: u32,
    pub destination_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderPdfPageResult {
    pub output_path: PathBuf,
    pub width_pixels: u32,
    pub height_pixels: u32,
}
