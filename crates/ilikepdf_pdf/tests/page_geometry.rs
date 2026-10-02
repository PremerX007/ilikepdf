use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ilikepdf_pdf::{
    PdfDpiRenderRequest, PdfErrorKind, PdfPageBox, PdfPageRotation, PdfRenderRequest, PdfRenderer,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn renderer() -> &'static PdfRenderer {
    static RENDERER: OnceLock<PdfRenderer> = OnceLock::new();
    RENDERER.get_or_init(|| {
        PdfRenderer::from_library_path(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../third_party/pdfium/windows/x64/pdfium.dll"),
        )
        .expect("vendored PDFium runtime should load")
    })
}

fn rect(left: f64, bottom: f64, right: f64, top: f64) -> PdfPageBox {
    PdfPageBox {
        left_points: left,
        bottom_points: bottom,
        right_points: right,
        top_points: top,
    }
}

#[test]
fn inspects_controlled_sizes_rotations_boxes_and_defaults_without_changing_source() {
    let source = fixture("editor_geometry.pdf");
    let before = fs::read(&source).unwrap();
    let document = renderer().inspect_page_geometry(&source).unwrap();
    assert_eq!(document.pages.len(), 12);
    assert!((document.pages[0].display_width_points - 595.2756).abs() < 0.001);
    assert!((document.pages[0].display_height_points - 841.8898).abs() < 0.001);
    assert_eq!(document.pages[1].visible_box, rect(0.0, 0.0, 612.0, 792.0));
    assert_eq!(document.pages[1].declared_crop_box, None);
    for (index, rotation) in [
        PdfPageRotation::None,
        PdfPageRotation::Clockwise90,
        PdfPageRotation::HalfTurn,
        PdfPageRotation::Clockwise270,
    ]
    .into_iter()
    .enumerate()
    {
        let page = &document.pages[index + 2];
        assert_eq!(
            page.declared_media_box,
            Some(rect(-50.0, -40.0, 350.0, 260.0))
        );
        assert_eq!(page.declared_crop_box, Some(rect(25.0, 30.0, 225.0, 180.0)));
        assert_eq!(page.visible_box, rect(25.0, 30.0, 225.0, 180.0));
        assert_eq!(page.rotation, rotation);
        let expected = if index % 2 == 0 {
            (200.0, 150.0)
        } else {
            (150.0, 200.0)
        };
        assert_eq!(
            (page.display_width_points, page.display_height_points),
            expected
        );
    }
    assert_eq!(document.pages[6].visible_box, rect(0.0, 20.0, 250.0, 200.0));
    assert_eq!(
        document.pages[6].declared_crop_box,
        Some(rect(-20.0, 20.0, 250.0, 240.0))
    );
    assert_eq!(
        document.pages[7].visible_box,
        rect(-100.0, -200.0, 200.0, 300.0)
    );
    assert_eq!(document.pages[8].visible_box, document.pages[3].visible_box);
    assert_eq!(document.pages[8].declared_media_box, None);
    assert_eq!(document.pages[8].declared_crop_box, None);
    assert_eq!(document.pages[8].rotation, PdfPageRotation::Clockwise90);
    // Missing dictionary entries must never be guessed from the rotated page size.
    assert_eq!(document.pages[9].declared_media_box, None);
    assert_eq!(document.pages[9].visible_box, rect(0.0, 0.0, 612.0, 792.0));
    assert_eq!(
        document.pages[10].declared_media_box,
        Some(rect(0.0, 0.0, 300.0, 200.0))
    );
    assert_eq!(
        document.pages[10].visible_box,
        rect(20.0, 30.0, 240.0, 170.0)
    );
    assert_eq!(document.pages[11].visible_box, rect(0.0, 0.0, 300.0, 200.0));
    assert_eq!(document.pages[11].declared_crop_box, None);
    assert_eq!(fs::read(source).unwrap(), before);
}

#[test]
fn rejects_disjoint_boxes_and_preserves_document_error_categories() {
    assert_eq!(
        renderer()
            .inspect_page_geometry(&fixture("editor_disjoint_boxes.pdf"))
            .unwrap_err()
            .kind,
        PdfErrorKind::InvalidDocument
    );
    assert_eq!(
        renderer()
            .inspect_page_geometry(&fixture("malformed.pdf"))
            .unwrap_err()
            .kind,
        PdfErrorKind::InvalidDocument
    );
    assert_eq!(
        renderer()
            .inspect_page_geometry(&fixture("missing-editor-fixture.pdf"))
            .unwrap_err()
            .kind,
        PdfErrorKind::SourceNotFound
    );
}

#[test]
fn bundled_pdfium_reports_identical_geometry_for_user_unit_one_and_two() {
    let one = fixture("editor_user_unit_1.pdf");
    let two = fixture("editor_user_unit_2.pdf");
    // Prove the fixtures isolate UserUnit rather than accidentally changing boxes
    // or content. Same-length numeric entries also leave xref offsets identical.
    let one_bytes = fs::read(&one).unwrap();
    let two_bytes = fs::read(&two).unwrap();
    let one_text = std::str::from_utf8(&one_bytes).unwrap();
    assert_eq!(one_text.matches("/UserUnit 1").count(), 4);
    assert_eq!(
        one_text.replace("/UserUnit 1", "/UserUnit 2").as_bytes(),
        two_bytes
    );

    let one_geometry = renderer().inspect_page_geometry(&one).unwrap();
    let two_geometry = renderer().inspect_page_geometry(&two).unwrap();
    assert_eq!(one_geometry, two_geometry);
    assert_eq!(one_geometry.pages.len(), 4);
    for (index, page) in one_geometry.pages.iter().enumerate() {
        assert_eq!(
            page.declared_media_box,
            Some(rect(-50.0, -40.0, 350.0, 260.0))
        );
        assert_eq!(page.declared_crop_box, Some(rect(25.0, 30.0, 225.0, 180.0)));
        assert_eq!(page.visible_box, rect(25.0, 30.0, 225.0, 180.0));
        let expected = if index % 2 == 0 {
            (200.0, 150.0)
        } else {
            (150.0, 200.0)
        };
        assert_eq!(
            (page.display_width_points, page.display_height_points),
            expected
        );
    }
    let one_info = renderer().inspect_document(&one).unwrap();
    assert_eq!(one_info, renderer().inspect_document(&two).unwrap());
    let size = one_info.first_page_size.unwrap();
    assert_eq!((size.width_points, size.height_points), (200.0, 150.0));
    assert_eq!(fs::read(one).unwrap(), one_bytes);
    assert_eq!(fs::read(two).unwrap(), two_bytes);
}

#[test]
fn bundled_pdfium_ignores_user_unit_consistently_in_dpi_and_width_rendering() {
    let one = fixture("editor_user_unit_1.pdf");
    let two = fixture("editor_user_unit_2.pdf");
    let one_bytes = fs::read(&one).unwrap();
    let two_bytes = fs::read(&two).unwrap();
    // Expected blue marker centers at 72 DPI, independently specified for
    // rotations 0/90/180/270 after the non-zero crop offset and y-axis inversion.
    let blue_centers = [(50.0, 115.0), (35.0, 50.0), (150.0, 35.0), (115.0, 150.0)];
    for (index, (blue_x, blue_y)) in blue_centers.into_iter().enumerate() {
        for dpi in [72, 144, 150] {
            let mut pair = Vec::new();
            for source in [&one, &two] {
                let mut png = Cursor::new(Vec::new());
                let rendered = renderer()
                    .render_page_to_png_at_dpi(
                        PdfDpiRenderRequest {
                            source_path: source.clone(),
                            page_index: index as u32,
                            dpi,
                        },
                        &mut png,
                    )
                    .unwrap();
                let dimensions = match (dpi, index % 2) {
                    (72, 0) => (200, 150),
                    (72, _) => (150, 200),
                    (144, 0) => (400, 300),
                    (144, _) => (300, 400),
                    (150, 0) => (417, 313),
                    (150, _) => (313, 417),
                    _ => unreachable!(),
                };
                assert_eq!((rendered.width_pixels, rendered.height_pixels), dimensions);
                let pixels =
                    image::load_from_memory_with_format(png.get_ref(), image::ImageFormat::Png)
                        .unwrap()
                        .into_rgba8();
                assert_eq!(pixels.dimensions(), dimensions);
                let (width_points, height_points) = if index % 2 == 0 {
                    (200.0, 150.0)
                } else {
                    (150.0, 200.0)
                };
                let marker_x = (blue_x * f64::from(dimensions.0) / width_points).floor() as u32;
                let marker_y = (blue_y * f64::from(dimensions.1) / height_points).floor() as u32;
                assert_eq!(pixels.get_pixel(marker_x, marker_y).0, [0, 0, 255, 255]);
                pair.push(pixels);
            }
            assert_eq!(
                pair[0], pair[1],
                "UserUnit changed DPI render on page {index}"
            );
        }
        let mut pair = Vec::new();
        for source in [&one, &two] {
            let mut png = Cursor::new(Vec::new());
            let rendered = renderer()
                .render_page_to_png(
                    PdfRenderRequest {
                        source_path: source.clone(),
                        page_index: index as u32,
                        target_width: 401,
                    },
                    &mut png,
                )
                .unwrap();
            assert_eq!(
                (rendered.width_pixels, rendered.height_pixels),
                if index % 2 == 0 {
                    (401, 301)
                } else {
                    (401, 535)
                }
            );
            pair.push(
                image::load_from_memory_with_format(png.get_ref(), image::ImageFormat::Png)
                    .unwrap()
                    .into_rgba8(),
            );
        }
        assert_eq!(
            pair[0], pair[1],
            "UserUnit changed width render on page {index}"
        );
    }
    assert_eq!(fs::read(one).unwrap(), one_bytes);
    assert_eq!(fs::read(two).unwrap(), two_bytes);
}
