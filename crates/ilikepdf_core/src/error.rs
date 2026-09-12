use std::error::Error;
use std::fmt::{self, Display, Formatter};

pub type ApplicationResult<T> = Result<T, ApplicationError>;

/// An application failure with a machine-readable code and safe display message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationError {
    pub code: ApplicationErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApplicationErrorCode {
    SourceNotFound,
    SourceNotFile,
    SourceUnreadable,
    InvalidPdf,
    PageOutOfBounds,
    InvalidRequest,
    PdfRuntimeUnavailable,
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

impl ApplicationError {
    pub(crate) fn new(code: ApplicationErrorCode, message: &'static str) -> Self {
        Self {
            code,
            message: message.to_owned(),
        }
    }
}

impl Display for ApplicationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ApplicationError {}

#[cfg(test)]
mod tests;
