use std::path::PathBuf;

use super::image_to_pdf::{ImagePdfMargin, ImagePdfOrientation, ImagePdfPageSize};
use super::pdf_export::{PdfBatchDestinationMode, PdfExportFormat, PdfExportQuality};

pub(super) fn paths_to_strings(paths: Vec<PathBuf>) -> Vec<String> {
    paths
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

impl From<ImagePdfPageSize> for ilikepdf_core::ImagePdfPageSize {
    fn from(value: ImagePdfPageSize) -> Self {
        match value {
            ImagePdfPageSize::Fit => Self::Fit,
            ImagePdfPageSize::A4 => Self::A4,
            ImagePdfPageSize::UsLetter => Self::UsLetter,
        }
    }
}

impl From<ImagePdfOrientation> for ilikepdf_core::ImagePdfOrientation {
    fn from(value: ImagePdfOrientation) -> Self {
        match value {
            ImagePdfOrientation::Portrait => Self::Portrait,
            ImagePdfOrientation::Landscape => Self::Landscape,
        }
    }
}

impl From<ImagePdfMargin> for ilikepdf_core::ImagePdfMargin {
    fn from(value: ImagePdfMargin) -> Self {
        match value {
            ImagePdfMargin::None => Self::None,
            ImagePdfMargin::Small => Self::Small,
            ImagePdfMargin::Big => Self::Big,
        }
    }
}

impl From<PdfExportQuality> for ilikepdf_core::PdfExportQuality {
    fn from(value: PdfExportQuality) -> Self {
        match value {
            PdfExportQuality::Standard => Self::Standard,
            PdfExportQuality::HighQuality => Self::HighQuality,
        }
    }
}

impl From<PdfExportFormat> for ilikepdf_core::PdfExportFormat {
    fn from(value: PdfExportFormat) -> Self {
        match value {
            PdfExportFormat::Png => Self::Png,
            PdfExportFormat::Jpg => Self::Jpg,
        }
    }
}

impl From<PdfBatchDestinationMode> for ilikepdf_core::PdfBatchDestinationMode {
    fn from(value: PdfBatchDestinationMode) -> Self {
        match value {
            PdfBatchDestinationMode::NextToSourceFiles => Self::NextToSourceFiles,
            PdfBatchDestinationMode::CustomFolder => Self::CustomFolder,
        }
    }
}
