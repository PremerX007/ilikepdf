use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use ilikepdf_core::{
    ApplicationErrorCode, RewriteStructuralPdfRequest, StructuralPdfEngine,
    StructuralPdfEngineFamily, StructuralPdfError, StructuralPdfVersion,
    probe_structural_pdf_engine, rewrite_structural_pdf, validate_structural_pdf,
};
use ilikepdf_pdf::{PdfRenderRequest, inspect_document, render_page_to_png};
use ilikepdf_qpdf::QpdfCliEngine;

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn qpdf_runtime_root() -> PathBuf {
    repository_root()
        .join("third_party")
        .join("qpdf")
        .join("windows")
        .join("x64")
}

fn pdfium_library() -> PathBuf {
    repository_root()
        .join("third_party")
        .join("pdfium")
        .join("windows")
        .join("x64")
        .join("pdfium.dll")
}

fn fixture(name: &str) -> PathBuf {
    repository_root()
        .join("crates")
        .join("ilikepdf_pdf")
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn official_runtime_probe_returns_the_pinned_typed_version() {
    let engine = QpdfCliEngine::from_runtime_root(qpdf_runtime_root());

    let info = probe_structural_pdf_engine(&engine).expect("vendored qpdf should be available");

    assert_eq!(info.family, StructuralPdfEngineFamily::Qpdf);
    assert_eq!(info.version, StructuralPdfVersion::new(12, 4, 1));
}

#[test]
fn real_qpdf_validation_accepts_valid_pdf_and_rejects_malformed_input() {
    let engine = QpdfCliEngine::from_runtime_root(qpdf_runtime_root());
    let valid_source = fixture("one_page.pdf");
    let malformed_source = fixture("malformed.pdf");
    let valid_before = fs::read(&valid_source).expect("valid fixture should be readable");
    let malformed_before =
        fs::read(&malformed_source).expect("malformed fixture should be readable");

    let validation = validate_structural_pdf(&engine, &valid_source)
        .expect("qpdf should accept the valid fixture");
    let error = validate_structural_pdf(&engine, &malformed_source)
        .expect_err("qpdf should reject malformed input");

    assert!(!validation.has_warnings);
    assert_eq!(error.code, ApplicationErrorCode::InvalidPdf);
    assert_eq!(fs::read(valid_source).unwrap(), valid_before);
    assert_eq!(fs::read(malformed_source).unwrap(), malformed_before);
}

#[test]
fn application_rewrite_handles_spaces_and_unicode_and_round_trips_through_pdfium() {
    install_pdfium_beside_test_executable();
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let unicode_directory = directory.path().join("พื้นที่ ทดสอบ qpdf");
    fs::create_dir(&unicode_directory).expect("Unicode test directory should be created");
    let source = unicode_directory.join("ต้นฉบับ สองหน้า.pdf");
    let destination = unicode_directory.join("ผลลัพธ์ เขียนใหม่.pdf");
    fs::copy(fixture("two_page.pdf"), &source).expect("fixture should be copied");
    let source_before = fs::read(&source).expect("source should be readable");
    let engine = QpdfCliEngine::from_runtime_root(qpdf_runtime_root());

    let result = rewrite_structural_pdf(
        &engine,
        RewriteStructuralPdfRequest {
            source_path: source.clone(),
            destination_path: destination.clone(),
        },
    )
    .expect("content-preserving rewrite should succeed");

    assert_eq!(result.output_path, destination);
    assert_eq!(result.page_count, 2);
    assert!(!result.has_warnings);
    assert!(result.output_path.metadata().unwrap().len() > 0);
    assert_eq!(fs::read(&source).unwrap(), source_before);
    assert_eq!(private_working_file_count(&unicode_directory), 0);

    let output_info = inspect_document(&result.output_path)
        .expect("rewritten output should reopen through PDFium");
    let mut rendered = Cursor::new(Vec::new());
    render_page_to_png(
        PdfRenderRequest {
            source_path: result.output_path,
            page_index: 0,
            target_width: 128,
        },
        &mut rendered,
    )
    .expect("a representative rewritten page should render");

    assert_eq!(output_info.page_count, 2);
    assert_eq!(&rendered.into_inner()[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn existing_destination_is_not_replaced_and_leaves_no_working_file() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let destination = directory.path().join("existing.pdf");
    fs::write(&destination, b"existing destination").unwrap();
    let engine = QpdfCliEngine::from_runtime_root(qpdf_runtime_root());

    let error = rewrite_structural_pdf(
        &engine,
        RewriteStructuralPdfRequest {
            source_path: fixture("one_page.pdf"),
            destination_path: destination.clone(),
        },
    )
    .expect_err("existing output must not be overwritten");

    assert_eq!(error.code, ApplicationErrorCode::OutputAlreadyExists);
    assert_eq!(fs::read(destination).unwrap(), b"existing destination");
    assert_eq!(private_working_file_count(directory.path()), 0);
}

#[test]
fn missing_bundled_runtime_maps_without_falling_back_to_path() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let engine = QpdfCliEngine::from_runtime_root(directory.path().join("absent"));

    assert_eq!(engine.probe(), Err(StructuralPdfError::RuntimeUnavailable));
    let error = probe_structural_pdf_engine(&engine)
        .expect_err("the application boundary should map the infrastructure error");
    assert_eq!(
        error.code,
        ApplicationErrorCode::StructuralPdfRuntimeUnavailable
    );
}

fn install_pdfium_beside_test_executable() {
    let executable = std::env::current_exe().expect("test executable should resolve");
    let destination = executable
        .parent()
        .expect("test executable should have a parent")
        .join("pdfium.dll");
    if !destination.is_file() {
        fs::copy(pdfium_library(), destination)
            .expect("PDFium should be copied beside the integration-test executable");
    }
}

fn private_working_file_count(directory: &Path) -> usize {
    fs::read_dir(directory)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains(".ilikepdf-structural-pdf-")
        })
        .count()
}
