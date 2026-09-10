use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ilikepdf_pdf::{PdfErrorKind, PdfRenderRequest, PdfRenderer};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn renderer() -> &'static PdfRenderer {
    static RENDERER: OnceLock<PdfRenderer> = OnceLock::new();
    RENDERER.get_or_init(|| {
        let library = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("third_party")
            .join("pdfium")
            .join("windows")
            .join("x64")
            .join("pdfium.dll");
        PdfRenderer::from_library_path(&library).expect("vendored PDFium runtime should load")
    })
}

#[test]
fn opens_fixture_and_reports_page_information() {
    let info = renderer()
        .inspect_document(&fixture("two_page.pdf"))
        .expect("fixture should open");

    assert_eq!(info.page_count, 2);
    let size = info.first_page_size.expect("fixture has a first page");
    assert!((size.width_points - 300.0).abs() < 0.01);
    assert!((size.height_points - 200.0).abs() < 0.01);
}

#[test]
fn renders_first_page_to_a_valid_png_without_changing_source() {
    let source = fixture("two_page.pdf");
    let before = fs::read(&source).expect("fixture should be readable");
    let mut output = Cursor::new(Vec::new());

    let result = renderer()
        .render_page_to_png(
            PdfRenderRequest {
                source_path: source.clone(),
                page_index: 0,
                target_width: 600,
            },
            &mut output,
        )
        .expect("page should render");

    assert_eq!(result.width_pixels, 600);
    assert_eq!(result.height_pixels, 400);
    assert_eq!(&output.into_inner()[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(
        fs::read(source).expect("fixture should remain readable"),
        before
    );
}

#[test]
fn rejects_page_index_outside_document() {
    let mut output = Cursor::new(Vec::new());
    let error = renderer()
        .render_page_to_png(
            PdfRenderRequest {
                source_path: fixture("two_page.pdf"),
                page_index: 2,
                target_width: 600,
            },
            &mut output,
        )
        .expect_err("page index two is outside a two-page document");

    assert_eq!(error.kind, PdfErrorKind::PageOutOfBounds);
}

#[test]
fn rejects_missing_source() {
    let error = renderer()
        .inspect_document(&fixture("missing.pdf"))
        .expect_err("a missing source should be rejected");

    assert_eq!(error.kind, PdfErrorKind::SourceNotFound);
}

#[test]
fn rejects_directory_source() {
    let error = renderer()
        .inspect_document(
            fixture("two_page.pdf")
                .parent()
                .expect("fixture has a parent"),
        )
        .expect_err("a directory should be rejected");

    assert_eq!(error.kind, PdfErrorKind::SourceNotFile);
}

#[test]
fn rejects_malformed_pdf() {
    let error = renderer()
        .inspect_document(&fixture("malformed.pdf"))
        .expect_err("malformed data should be rejected");

    assert_eq!(error.kind, PdfErrorKind::InvalidDocument);
}
