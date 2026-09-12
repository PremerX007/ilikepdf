use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ilikepdf_pdf::{
    PdfDpiRenderRequest, PdfErrorKind, PdfImageFormat, PdfRenderRequest, PdfRenderer,
};
use image::GenericImageView;

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
fn renders_each_page_at_150_dpi_using_its_point_dimensions() {
    let source = fixture("two_page.pdf");

    let first = render_at_dpi(&source, 0, 150);
    let second = render_at_dpi(&source, 1, 150);

    assert_eq!(first, (625, 417));
    assert_eq!(second, (417, 625));
}

#[test]
fn renders_300_dpi_larger_than_150_dpi_with_the_same_aspect_ratio() {
    let source = fixture("two_page.pdf");

    let standard = render_at_dpi(&source, 0, 150);
    let high_quality = render_at_dpi(&source, 0, 300);

    assert_eq!(high_quality, (1250, 833));
    assert!(high_quality.0 > standard.0);
    assert!(high_quality.1 > standard.1);
    assert!((standard.0 as f64 / standard.1 as f64 - 1.5).abs() < 0.01);
    assert!((high_quality.0 as f64 / high_quality.1 as f64 - 1.5).abs() < 0.01);
}

#[test]
fn renders_a_page_to_a_valid_jpg_at_the_requested_dpi() {
    let source = fixture("two_page.pdf");
    let before = fs::read(&source).expect("fixture should be readable");
    let mut output = Cursor::new(Vec::new());

    let rendered = renderer()
        .render_page_to_image_at_dpi(
            PdfDpiRenderRequest {
                source_path: source.clone(),
                page_index: 0,
                dpi: 150,
            },
            PdfImageFormat::Jpg,
            &mut output,
        )
        .expect("page should render as JPG");
    let bytes = output.into_inner();
    let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)
        .expect("rendered bytes should decode as JPG");

    assert_eq!(&bytes[..3], b"\xff\xd8\xff");
    assert_eq!(decoded.dimensions(), (625, 417));
    assert_eq!(
        decoded.dimensions(),
        (rendered.width_pixels, rendered.height_pixels)
    );
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

fn render_at_dpi(source: &Path, page_index: u32, dpi: u16) -> (u32, u32) {
    let mut output = Cursor::new(Vec::new());
    let rendered = renderer()
        .render_page_to_png_at_dpi(
            PdfDpiRenderRequest {
                source_path: source.to_path_buf(),
                page_index,
                dpi,
            },
            &mut output,
        )
        .expect("page should render at the requested DPI");
    let decoded =
        image::load_from_memory_with_format(&output.into_inner(), image::ImageFormat::Png)
            .expect("rendered bytes should decode as PNG");

    assert_eq!(
        decoded.dimensions(),
        (rendered.width_pixels, rendered.height_pixels)
    );
    decoded.dimensions()
}
