/// Application metadata exposed to Dart through a typed bridge contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationInfo {
    pub name: String,
    pub version: String,
    pub local_only: bool,
}

pub use super::error::{ApplicationError, ApplicationErrorCode};

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
mod tests;
