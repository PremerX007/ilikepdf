use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PdfErrorKind {
    SourceNotFound,
    SourceNotFile,
    SourceUnreadable,
    InvalidDocument,
    PageOutOfBounds,
    RuntimeUnavailable,
    RenderFailed,
    EncodeFailed,
    OutputWriteFailed,
    UnsupportedImageFormat,
    MalformedImage,
    ImageDecodeFailed,
    ImageOrientationFailed,
    DocumentCreateFailed,
    PageCreateFailed,
    ImagePlacementFailed,
    SaveFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfError {
    pub kind: PdfErrorKind,
}

impl PdfError {
    pub(crate) const fn new(kind: PdfErrorKind) -> Self {
        Self { kind }
    }
}

impl Display for PdfError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            PdfErrorKind::SourceNotFound => "PDF source not found",
            PdfErrorKind::SourceNotFile => "PDF source is not a file",
            PdfErrorKind::SourceUnreadable => "PDF source is unreadable",
            PdfErrorKind::InvalidDocument => "invalid PDF document",
            PdfErrorKind::PageOutOfBounds => "PDF page index is out of bounds",
            PdfErrorKind::RuntimeUnavailable => "PDF runtime unavailable",
            PdfErrorKind::RenderFailed => "PDF render failed",
            PdfErrorKind::EncodeFailed => "image encoding failed",
            PdfErrorKind::OutputWriteFailed => "image output write failed",
            PdfErrorKind::UnsupportedImageFormat => "unsupported image format",
            PdfErrorKind::MalformedImage => "malformed image",
            PdfErrorKind::ImageDecodeFailed => "image decode failed",
            PdfErrorKind::ImageOrientationFailed => "image orientation processing failed",
            PdfErrorKind::DocumentCreateFailed => "PDF document creation failed",
            PdfErrorKind::PageCreateFailed => "PDF page creation failed",
            PdfErrorKind::ImagePlacementFailed => "PDF image placement failed",
            PdfErrorKind::SaveFailed => "PDF save failed",
        })
    }
}

impl Error for PdfError {}
