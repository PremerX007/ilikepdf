use std::fs;

use super::*;

#[test]
fn missing_runtime_is_structured_and_never_falls_back_to_path() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let error = QpdfRuntimeResolver::from_root(directory.path().join("missing"))
        .resolve()
        .expect_err("missing bundled runtime should fail");

    assert_eq!(error, StructuralPdfError::RuntimeUnavailable);
}

#[test]
fn application_executable_resolves_the_relocatable_runtime_layout() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let application = directory.path().join("ilikepdf.exe");
    fs::write(&application, b"test application").unwrap();

    let resolver = QpdfRuntimeResolver::from_application_executable(&application);
    let expected_root = directory.path().join("runtime").join("qpdf");

    assert_eq!(
        resolver
            .runtime_root()
            .expect("runtime root should resolve"),
        expected_root
    );
}
