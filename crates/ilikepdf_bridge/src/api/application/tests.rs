use super::*;

#[test]
fn maps_core_metadata_to_the_bridge_contract() {
    let info = get_application_info().expect("static application metadata should be valid");

    assert_eq!(info.name, "iLikePDF");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert!(info.local_only);
}
