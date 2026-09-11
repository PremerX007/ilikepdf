/// Application metadata exposed to Dart through a typed bridge contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationInfo {
    pub name: String,
    pub version: String,
    pub local_only: bool,
}

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
    InvalidOutputDirectory,
    PermissionDenied,
    OutputNotWritable,
    OutputAlreadyExists,
    OutputWriteFailed,
    RenderingFailed,
    EncodingFailed,
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
            ilikepdf_core::ApplicationErrorCode::Internal => ApplicationErrorCode::Internal,
            _ => ApplicationErrorCode::Internal,
        };

        Self {
            code,
            message: error.message,
        }
    }
}

/// Runs on flutter_rust_bridge's worker pool so Dart's UI isolate stays responsive.
pub fn get_application_info() -> Result<ApplicationInfo, ApplicationError> {
    crate::logging::record(crate::logging::Event::ApplicationInfoRequested);

    ilikepdf_core::get_application_info()
        .map(|info| ApplicationInfo {
            name: info.name,
            version: info.version,
            local_only: info.local_only,
        })
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_core_metadata_to_the_bridge_contract() {
        let info = get_application_info().expect("static application metadata should be valid");

        assert_eq!(info.name, "ilikepdf");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.local_only);
    }
}
