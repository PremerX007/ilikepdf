use std::fs;
use std::path::{Path, PathBuf};

use ilikepdf_core::{StructuralPdfEngine, StructuralPdfError, StructuralPdfVersion};
use ilikepdf_pdf::PdfRenderer;
use ilikepdf_qpdf::QpdfCliEngine;

#[test]
#[ignore = "requires ILIKEPDF_RELEASE_EXECUTABLE from a completed Windows release build"]
fn release_bundle_resolves_both_native_runtimes() {
    let application = PathBuf::from(
        std::env::var_os("ILIKEPDF_RELEASE_EXECUTABLE")
            .expect("ILIKEPDF_RELEASE_EXECUTABLE must point to the release executable"),
    );
    assert!(application.is_file(), "release application should exist");
    let release_directory = application
        .parent()
        .expect("release application should have a directory");
    let qpdf_root = release_directory.join("runtime").join("qpdf");

    verify_manifest_files(&qpdf_root);
    PdfRenderer::from_library_path(&release_directory.join("pdfium.dll"))
        .expect("release PDFium should load");
    let info = QpdfCliEngine::from_application_executable(&application)
        .probe()
        .expect("release qpdf should resolve and launch without PATH fallback");

    assert_eq!(info.version, StructuralPdfVersion::new(12, 4, 1));

    let isolated = tempfile::tempdir().expect("isolated application directory should be created");
    let isolated_application = isolated.path().join("ilikepdf.exe");
    fs::copy(&application, &isolated_application)
        .expect("release executable should be copied without its runtime");
    assert_eq!(
        QpdfCliEngine::from_application_executable(isolated_application).probe(),
        Err(StructuralPdfError::RuntimeUnavailable),
        "the resolver must not fall back to a qpdf installation on PATH"
    );
}

fn verify_manifest_files(runtime_root: &Path) {
    let manifest_path = runtime_root.join("runtime-manifest.txt");
    let manifest = fs::read_to_string(&manifest_path)
        .expect("release qpdf runtime manifest should be present");
    for line in manifest.lines() {
        let relative_path = ["runtime_file=", "package_file=", "provenance_file="]
            .iter()
            .find_map(|prefix| line.strip_prefix(prefix))
            .and_then(|value| value.split_once('|').map(|(path, _)| path));
        if let Some(relative_path) = relative_path {
            assert!(
                runtime_root.join(relative_path).is_file(),
                "release qpdf component is missing: {relative_path}"
            );
        }
    }
}
