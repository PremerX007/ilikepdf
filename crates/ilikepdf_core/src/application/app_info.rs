use crate::error::ApplicationResult;

/// Stable metadata supplied by the Rust application core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationInfo {
    pub name: String,
    pub version: String,
    pub local_only: bool,
}

/// Returns core metadata through the same fallible boundary used by future operations.
pub fn get_application_info() -> ApplicationResult<ApplicationInfo> {
    Ok(ApplicationInfo {
        name: "ilikepdf".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        local_only: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_version_and_local_only_contract() {
        let info = get_application_info().expect("static application metadata should be valid");

        assert_eq!(info.name, "ilikepdf");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.local_only);
    }
}
