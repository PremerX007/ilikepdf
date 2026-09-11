use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ilikepdf_pdf::{
    ImagePdfLayout, ImagePdfMargin, ImagePdfOrientation, ImagePdfPageSize, ImagePdfRequest,
    PdfErrorKind, PdfRenderRequest, PdfRenderer,
};
use image::GenericImageView;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("images")
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

fn layout(page_size: ImagePdfPageSize) -> ImagePdfLayout {
    ImagePdfLayout {
        page_size,
        orientation: ImagePdfOrientation::Portrait,
        margin: ImagePdfMargin::None,
    }
}

fn create_pdf(
    source_paths: Vec<PathBuf>,
    layout: ImagePdfLayout,
) -> (ilikepdf_pdf::ImagePdfResult, Vec<u8>) {
    let mut output = Cursor::new(Vec::new());
    let result = renderer()
        .create_image_pdf(
            ImagePdfRequest {
                source_paths,
                layout,
            },
            &mut output,
            |_| {},
        )
        .expect("fixture images should create a PDF");
    (result, output.into_inner())
}

fn write_pdf(bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let path = directory.path().join("generated.pdf");
    fs::write(&path, bytes).expect("generated PDF should be written");
    (directory, path)
}

#[test]
fn supported_formats_decode_and_create_one_page_pdfs() {
    for name in ["photo.jpg", "photo.jpeg", "portrait.png", "sample.webp"] {
        let (_, bytes) = create_pdf(vec![fixture(name)], layout(ImagePdfPageSize::A4));
        let (_directory, path) = write_pdf(&bytes);
        let info = renderer()
            .inspect_document(&path)
            .expect("generated PDF should reopen");
        assert_eq!(info.page_count, 1, "fixture {name}");
    }
}

#[test]
fn ordinary_jpeg_uses_pdfiums_inline_jpeg_embedding_path() {
    let (_, bytes) = create_pdf(vec![fixture("photo.jpg")], layout(ImagePdfPageSize::A4));

    assert!(
        bytes
            .windows(b"/DCTDecode".len())
            .any(|window| window == b"/DCTDecode"),
        "the generated PDF should retain JPEG DCT data"
    );
}

#[test]
fn malformed_and_unsupported_images_return_structured_errors() {
    let malformed = renderer()
        .inspect_image(&fixture("malformed.png"))
        .expect_err("malformed PNG should fail");
    assert!(matches!(
        malformed.kind,
        PdfErrorKind::MalformedImage | PdfErrorKind::ImageDecodeFailed
    ));

    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let unsupported = directory.path().join("portrait.bmp");
    fs::copy(fixture("portrait.png"), &unsupported).expect("fixture should copy");
    let error = renderer()
        .inspect_image(&unsupported)
        .expect_err("unsupported extension should fail");
    assert_eq!(error.kind, PdfErrorKind::UnsupportedImageFormat);
}

#[test]
fn animated_png_and_webp_are_rejected_instead_of_silently_using_one_frame() {
    for name in ["animated.png", "animated.webp"] {
        let error = renderer()
            .inspect_image(&fixture(name))
            .expect_err("animated input should be unsupported");
        assert_eq!(error.kind, PdfErrorKind::UnsupportedImageFormat);
    }
}

#[test]
fn merged_pdf_preserves_input_order_and_round_trips_through_pdfium() {
    let (_, bytes) = create_pdf(
        vec![
            fixture("portrait.png"),
            fixture("landscape.png"),
            fixture("sample.webp"),
        ],
        layout(ImagePdfPageSize::Fit),
    );
    let (_directory, path) = write_pdf(&bytes);
    let info = renderer()
        .inspect_document(&path)
        .expect("generated PDF should reopen");
    assert_eq!(info.page_count, 3);

    let expected_center_colors = [(220, 30, 30), (30, 180, 70), (135, 45, 190)];
    for (page_index, expected) in expected_center_colors.into_iter().enumerate() {
        let mut rendered = Cursor::new(Vec::new());
        renderer()
            .render_page_to_png(
                PdfRenderRequest {
                    source_path: path.clone(),
                    page_index: u32::try_from(page_index).expect("small index"),
                    target_width: 240,
                },
                &mut rendered,
            )
            .expect("generated page should render");
        let image = image::load_from_memory(&rendered.into_inner()).expect("PNG should decode");
        let pixel = image.get_pixel(image.width() / 2, image.height() / 2);
        assert!((i16::from(pixel[0]) - expected.0).abs() < 12);
        assert!((i16::from(pixel[1]) - expected.1).abs() < 12);
        assert!((i16::from(pixel[2]) - expected.2).abs() < 12);
    }
}

#[test]
fn fit_follows_visual_orientation_and_ignores_other_controls() {
    let (result, bytes) = create_pdf(
        vec![fixture("oriented.jpg")],
        ImagePdfLayout {
            page_size: ImagePdfPageSize::Fit,
            orientation: ImagePdfOrientation::Landscape,
            margin: ImagePdfMargin::Big,
        },
    );
    let page = &result.pages[0];
    assert!((page.width_points - 60.0).abs() < 0.01);
    assert!((page.height_points - 90.0).abs() < 0.01);
    assert_eq!(page.image_left_points, 0.0);
    assert_eq!(page.image_bottom_points, 0.0);
    assert_eq!(page.image_width_points, page.width_points);
    assert_eq!(page.image_height_points, page.height_points);

    let (_directory, path) = write_pdf(&bytes);
    let mut rendered = Cursor::new(Vec::new());
    let output = renderer()
        .render_page_to_png(
            PdfRenderRequest {
                source_path: path,
                page_index: 0,
                target_width: 120,
            },
            &mut rendered,
        )
        .expect("oriented page should render");
    assert!(output.height_pixels > output.width_pixels);
}

#[test]
fn a4_letter_and_margins_have_canonical_geometry() {
    let cases = [
        (
            ImagePdfPageSize::A4,
            ImagePdfOrientation::Portrait,
            595.2756,
            841.8898,
        ),
        (
            ImagePdfPageSize::A4,
            ImagePdfOrientation::Landscape,
            841.8898,
            595.2756,
        ),
        (
            ImagePdfPageSize::UsLetter,
            ImagePdfOrientation::Portrait,
            612.0,
            792.0,
        ),
        (
            ImagePdfPageSize::UsLetter,
            ImagePdfOrientation::Landscape,
            792.0,
            612.0,
        ),
    ];
    for (page_size, orientation, expected_width, expected_height) in cases {
        let (result, _) = create_pdf(
            vec![fixture("landscape.png")],
            ImagePdfLayout {
                page_size,
                orientation,
                margin: ImagePdfMargin::None,
            },
        );
        let page = &result.pages[0];
        assert!((page.width_points - expected_width).abs() < 0.001);
        assert!((page.height_points - expected_height).abs() < 0.001);
        assert!((page.image_width_points / page.image_height_points - 1.5).abs() < 0.001);
        assert!(
            (page.image_left_points * 2.0 + page.image_width_points - page.width_points).abs()
                < 0.001
        );
        assert!(
            (page.image_bottom_points * 2.0 + page.image_height_points - page.height_points).abs()
                < 0.001
        );
    }

    for (margin, expected_points) in [
        (ImagePdfMargin::None, 0.0),
        (ImagePdfMargin::Small, 28.346_457),
        (ImagePdfMargin::Big, 56.692_913),
    ] {
        let (result, _) = create_pdf(
            vec![fixture("landscape.png")],
            ImagePdfLayout {
                page_size: ImagePdfPageSize::A4,
                orientation: ImagePdfOrientation::Landscape,
                margin,
            },
        );
        let page = &result.pages[0];
        assert!(page.image_left_points + 0.001 >= expected_points);
        assert!(page.image_bottom_points + 0.001 >= expected_points);
        assert!(
            page.image_left_points + page.image_width_points
                <= page.width_points - expected_points + 0.001
        );
        assert!(
            page.image_bottom_points + page.image_height_points
                <= page.height_points - expected_points + 0.001
        );
    }
}

#[test]
fn transparent_pixels_render_against_the_normal_white_page() {
    let (_, bytes) = create_pdf(
        vec![fixture("transparent.png")],
        layout(ImagePdfPageSize::Fit),
    );
    let (_directory, path) = write_pdf(&bytes);
    let mut rendered = Cursor::new(Vec::new());
    renderer()
        .render_page_to_png(
            PdfRenderRequest {
                source_path: path,
                page_index: 0,
                target_width: 200,
            },
            &mut rendered,
        )
        .expect("transparent page should render");
    let image = image::load_from_memory(&rendered.into_inner()).expect("PNG should decode");
    let corner = image.get_pixel(5, 5);
    assert!(corner[0] > 245 && corner[1] > 245 && corner[2] > 245);
}
