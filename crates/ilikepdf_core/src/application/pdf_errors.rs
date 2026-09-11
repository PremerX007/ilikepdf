use ilikepdf_pdf::{PdfError, PdfErrorKind};

use crate::{ApplicationError, ApplicationErrorCode};

impl From<PdfError> for ApplicationError {
    fn from(error: PdfError) -> Self {
        let (code, message) = match error.kind {
            PdfErrorKind::SourceNotFound => (
                ApplicationErrorCode::SourceNotFound,
                "The selected PDF no longer exists",
            ),
            PdfErrorKind::SourceNotFile => (
                ApplicationErrorCode::SourceNotFile,
                "The selected path is not a file",
            ),
            PdfErrorKind::SourceUnreadable => (
                ApplicationErrorCode::SourceUnreadable,
                "The selected PDF could not be read",
            ),
            PdfErrorKind::InvalidDocument => (
                ApplicationErrorCode::InvalidPdf,
                "The selected file is not a valid readable PDF",
            ),
            PdfErrorKind::PageOutOfBounds => (
                ApplicationErrorCode::PageOutOfBounds,
                "The requested page does not exist",
            ),
            PdfErrorKind::RuntimeUnavailable => (
                ApplicationErrorCode::PdfRuntimeUnavailable,
                "The local PDF rendering runtime is unavailable",
            ),
            PdfErrorKind::RenderFailed => (
                ApplicationErrorCode::RenderingFailed,
                "The PDF page could not be rendered",
            ),
            PdfErrorKind::EncodeFailed => (
                ApplicationErrorCode::EncodingFailed,
                "The rendered page could not be encoded as PNG",
            ),
            PdfErrorKind::OutputWriteFailed => (
                ApplicationErrorCode::OutputWriteFailed,
                "The PNG data could not be written",
            ),
            _ => (
                ApplicationErrorCode::Internal,
                "Unable to complete the PDF operation",
            ),
        };

        ApplicationError::new(code, message)
    }
}
