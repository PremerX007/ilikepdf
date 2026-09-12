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
                "The rendered page could not be encoded as an image",
            ),
            PdfErrorKind::OutputWriteFailed => (
                ApplicationErrorCode::OutputWriteFailed,
                "The image data could not be written",
            ),
            PdfErrorKind::UnsupportedImageFormat => (
                ApplicationErrorCode::UnsupportedImageFormat,
                "Select a JPG, JPEG, PNG, or WebP image",
            ),
            PdfErrorKind::MalformedImage => (
                ApplicationErrorCode::MalformedImage,
                "The selected image is malformed or corrupted",
            ),
            PdfErrorKind::ImageDecodeFailed => (
                ApplicationErrorCode::ImageDecodeFailed,
                "The selected image could not be decoded",
            ),
            PdfErrorKind::ImageOrientationFailed => (
                ApplicationErrorCode::ImageOrientationFailed,
                "The selected image orientation could not be processed",
            ),
            PdfErrorKind::DocumentCreateFailed => (
                ApplicationErrorCode::PdfDocumentCreationFailed,
                "The PDF document could not be created",
            ),
            PdfErrorKind::PageCreateFailed => (
                ApplicationErrorCode::PdfPageCreationFailed,
                "A PDF page could not be created",
            ),
            PdfErrorKind::ImagePlacementFailed => (
                ApplicationErrorCode::ImagePlacementFailed,
                "An image could not be placed on its PDF page",
            ),
            PdfErrorKind::SaveFailed => (
                ApplicationErrorCode::PdfSaveFailed,
                "The PDF document could not be saved",
            ),
            _ => (
                ApplicationErrorCode::Internal,
                "Unable to complete the PDF operation",
            ),
        };

        ApplicationError::new(code, message)
    }
}
