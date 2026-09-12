use super::*;

#[test]
fn pinned_manifest_has_a_typed_version_and_executable_checksum() {
    let manifest = runtime_manifest().expect("pinned manifest should parse");

    assert_eq!(manifest.version, StructuralPdfVersion::new(12, 4, 1));
    assert_eq!(manifest.executable, PathBuf::from("bin/qpdf.exe"));
    assert!(
        manifest
            .runtime_files
            .iter()
            .all(|file| file.sha256.len() == 64)
    );
}
