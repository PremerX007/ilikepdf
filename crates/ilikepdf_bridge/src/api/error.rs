/// Structured errors exposed to Dart. Add error codes instead of returning raw strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationError {
    pub code: ApplicationErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationErrorCode {
    SourceNotFound,
    SourceNotFile,
    SourceUnreadable,
    InvalidPdf,
    PageOutOfBounds,
    InvalidRequest,
    PdfRuntimeUnavailable,
    StructuralPdfRuntimeUnavailable,
    StructuralPdfRuntimeIncompatible,
    StructuralPdfLaunchFailed,
    PasswordRequired,
    StructuralPdfOperationFailed,
    StructuralPdfOutputValidationFailed,
    InvalidOutputDirectory,
    PermissionDenied,
    OutputNotWritable,
    OutputAlreadyExists,
    OutputWriteFailed,
    RenderingFailed,
    EncodingFailed,
    UnsupportedImageFormat,
    MalformedImage,
    ImageDecodeFailed,
    ImageOrientationFailed,
    DuplicateOutputName,
    PdfDocumentCreationFailed,
    PdfPageCreationFailed,
    ImagePlacementFailed,
    PdfSaveFailed,
    Internal,
}

impl From<ilikepdf_core::ApplicationError> for ApplicationError {
    fn from(error: ilikepdf_core::ApplicationError) -> Self {
        let code = match error.code {
            ilikepdf_core::ApplicationErrorCode::SourceNotFound => {
                ApplicationErrorCode::SourceNotFound
            }
            ilikepdf_core::ApplicationErrorCode::SourceNotFile => {
                ApplicationErrorCode::SourceNotFile
            }
            ilikepdf_core::ApplicationErrorCode::SourceUnreadable => {
                ApplicationErrorCode::SourceUnreadable
            }
            ilikepdf_core::ApplicationErrorCode::InvalidPdf => ApplicationErrorCode::InvalidPdf,
            ilikepdf_core::ApplicationErrorCode::PageOutOfBounds => {
                ApplicationErrorCode::PageOutOfBounds
            }
            ilikepdf_core::ApplicationErrorCode::InvalidRequest => {
                ApplicationErrorCode::InvalidRequest
            }
            ilikepdf_core::ApplicationErrorCode::PdfRuntimeUnavailable => {
                ApplicationErrorCode::PdfRuntimeUnavailable
            }
            ilikepdf_core::ApplicationErrorCode::StructuralPdfRuntimeUnavailable => {
                ApplicationErrorCode::StructuralPdfRuntimeUnavailable
            }
            ilikepdf_core::ApplicationErrorCode::StructuralPdfRuntimeIncompatible => {
                ApplicationErrorCode::StructuralPdfRuntimeIncompatible
            }
            ilikepdf_core::ApplicationErrorCode::StructuralPdfLaunchFailed => {
                ApplicationErrorCode::StructuralPdfLaunchFailed
            }
            ilikepdf_core::ApplicationErrorCode::PasswordRequired => {
                ApplicationErrorCode::PasswordRequired
            }
            ilikepdf_core::ApplicationErrorCode::StructuralPdfOperationFailed => {
                ApplicationErrorCode::StructuralPdfOperationFailed
            }
            ilikepdf_core::ApplicationErrorCode::StructuralPdfOutputValidationFailed => {
                ApplicationErrorCode::StructuralPdfOutputValidationFailed
            }
            ilikepdf_core::ApplicationErrorCode::InvalidOutputDirectory => {
                ApplicationErrorCode::InvalidOutputDirectory
            }
            ilikepdf_core::ApplicationErrorCode::PermissionDenied => {
                ApplicationErrorCode::PermissionDenied
            }
            ilikepdf_core::ApplicationErrorCode::OutputNotWritable => {
                ApplicationErrorCode::OutputNotWritable
            }
            ilikepdf_core::ApplicationErrorCode::OutputAlreadyExists => {
                ApplicationErrorCode::OutputAlreadyExists
            }
            ilikepdf_core::ApplicationErrorCode::OutputWriteFailed => {
                ApplicationErrorCode::OutputWriteFailed
            }
            ilikepdf_core::ApplicationErrorCode::RenderingFailed => {
                ApplicationErrorCode::RenderingFailed
            }
            ilikepdf_core::ApplicationErrorCode::EncodingFailed => {
                ApplicationErrorCode::EncodingFailed
            }
            ilikepdf_core::ApplicationErrorCode::UnsupportedImageFormat => {
                ApplicationErrorCode::UnsupportedImageFormat
            }
            ilikepdf_core::ApplicationErrorCode::MalformedImage => {
                ApplicationErrorCode::MalformedImage
            }
            ilikepdf_core::ApplicationErrorCode::ImageDecodeFailed => {
                ApplicationErrorCode::ImageDecodeFailed
            }
            ilikepdf_core::ApplicationErrorCode::ImageOrientationFailed => {
                ApplicationErrorCode::ImageOrientationFailed
            }
            ilikepdf_core::ApplicationErrorCode::DuplicateOutputName => {
                ApplicationErrorCode::DuplicateOutputName
            }
            ilikepdf_core::ApplicationErrorCode::PdfDocumentCreationFailed => {
                ApplicationErrorCode::PdfDocumentCreationFailed
            }
            ilikepdf_core::ApplicationErrorCode::PdfPageCreationFailed => {
                ApplicationErrorCode::PdfPageCreationFailed
            }
            ilikepdf_core::ApplicationErrorCode::ImagePlacementFailed => {
                ApplicationErrorCode::ImagePlacementFailed
            }
            ilikepdf_core::ApplicationErrorCode::PdfSaveFailed => {
                ApplicationErrorCode::PdfSaveFailed
            }
            ilikepdf_core::ApplicationErrorCode::Internal => ApplicationErrorCode::Internal,
            _ => ApplicationErrorCode::Internal,
        };

        Self {
            code,
            message: error.message,
        }
    }
}
