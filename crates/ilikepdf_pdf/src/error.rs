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
            PdfErrorKind::EncodeFailed => "PNG encoding failed",
            PdfErrorKind::OutputWriteFailed => "PNG output write failed",
        })
    }
}

impl Error for PdfError {}
