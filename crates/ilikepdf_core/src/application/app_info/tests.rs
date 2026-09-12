use super::*;

#[test]
fn reports_version_and_local_only_contract() {
    let info = get_application_info().expect("static application metadata should be valid");

    assert_eq!(info.name, "iLikePDF");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert!(info.local_only);
}
