use std::fs;
use std::io::Cursor;

use ilikepdf_pdf::{PdfDpiRenderRequest, PdfRenderRequest};

use super::*;
use crate::{PageTransform, PdfPoint, ViewportRect};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ilikepdf_pdf/tests/fixtures")
        .join(name)
}

fn open_fixture(name: &str) -> ApplicationResult<EditorSession> {
    open_with_inspector(&fixture(name), |path| {
        crate::test_support::pdf_renderer().inspect_page_geometry(path)
    })
}

#[test]
fn creates_read_only_ordered_sessions_from_real_pdfium_geometry() {
    let source = fixture("editor_geometry.pdf");
    let before = fs::read(&source).unwrap();
    let session = open_fixture("editor_geometry.pdf").expect("geometry fixture opens");
    assert_eq!(session.source_path(), source);
    assert_eq!(session.state(), EditorSessionState::ReadOnly);
    assert_eq!(session.page_count(), 12);
    assert_eq!(
        session
            .pages()
            .iter()
            .map(|page| page.page_index)
            .collect::<Vec<_>>(),
        (0..12).collect::<Vec<_>>()
    );
    assert!((session.page(0).unwrap().geometry.width_points() - 595.2756).abs() < 0.001);
    assert!((session.page(0).unwrap().geometry.height_points() - 841.8898).abs() < 0.001);
    assert_eq!(session.page(1).unwrap().geometry.width_points(), 612.0);
    assert_eq!(session.page(1).unwrap().geometry.height_points(), 792.0);
    for (index, rotation) in [
        PageRotation::None,
        PageRotation::Clockwise90,
        PageRotation::HalfTurn,
        PageRotation::Clockwise270,
    ]
    .into_iter()
    .enumerate()
    {
        let page = session.page(index as u32 + 2).unwrap();
        assert_eq!(page.geometry.rotation(), rotation);
        assert_eq!(
            page.geometry.visible_box(),
            PageBox::new(25.0, 30.0, 225.0, 180.0).unwrap()
        );
    }
    assert_eq!(
        session.page(6).unwrap().geometry.visible_box(),
        PageBox::new(0.0, 20.0, 250.0, 200.0).unwrap()
    );
    assert_eq!(
        session.page(8).unwrap().geometry.rotation(),
        PageRotation::Clockwise90
    );
    assert_eq!(
        session.page(8).unwrap().geometry.visible_box(),
        session.page(3).unwrap().geometry.visible_box()
    );
    assert_eq!(
        session.page(9).unwrap().geometry.visible_box(),
        PageBox::new(0.0, 0.0, 612.0, 792.0).unwrap()
    );
    assert_eq!(
        session.page(12).unwrap_err().code,
        ApplicationErrorCode::PageOutOfBounds
    );
    assert_eq!(
        session.page(u32::MAX).unwrap_err().code,
        ApplicationErrorCode::PageOutOfBounds
    );
    let second = open_fixture("editor_geometry.pdf").unwrap();
    assert_ne!(second.id(), session.id());
    assert!(session.id().value() > 0);
    assert_eq!(session.clone().id(), session.id());
    assert_eq!(fs::read(source).unwrap(), before);
}

#[test]
fn maps_known_pdf_points_to_rendered_markers_for_every_rotation_and_inherited_boxes() {
    let source = fixture("editor_geometry.pdf");
    let before = fs::read(&source).unwrap();
    let session = open_fixture("editor_geometry.pdf").unwrap();
    // Real PDFium rendering is the independent oracle for origin/rotation/box behavior.
    for index in 2..12 {
        for dpi in [72, 144, 150] {
            let mut png = Cursor::new(Vec::new());
            let rendered = crate::test_support::pdf_renderer()
                .render_page_to_png_at_dpi(
                    PdfDpiRenderRequest {
                        source_path: source.clone(),
                        page_index: index,
                        dpi,
                    },
                    &mut png,
                )
                .unwrap();
            assert_markers(
                &session,
                index,
                rendered.width_pixels,
                rendered.height_pixels,
                png.get_ref(),
            );
        }
    }
    // Width-based rendering rounds raster dimensions independently, too.
    for index in 2..6 {
        let mut png = Cursor::new(Vec::new());
        let rendered = crate::test_support::pdf_renderer()
            .render_page_to_png(
                PdfRenderRequest {
                    source_path: source.clone(),
                    page_index: index,
                    target_width: 401,
                },
                &mut png,
            )
            .unwrap();
        assert_markers(
            &session,
            index,
            rendered.width_pixels,
            rendered.height_pixels,
            png.get_ref(),
        );
    }
    assert_eq!(fs::read(source).unwrap(), before);
}

#[test]
fn existing_transform_aligns_both_user_unit_variants_with_real_pdfium_rendering() {
    let one = open_fixture("editor_user_unit_1.pdf").unwrap();
    let two = open_fixture("editor_user_unit_2.pdf").unwrap();
    assert_eq!(one.page_count(), 4);
    assert_eq!(one.pages(), two.pages());
    let blue_centers = [(50.0, 115.0), (35.0, 50.0), (150.0, 35.0), (115.0, 150.0)];
    for session in [&one, &two] {
        let before = fs::read(session.source_path()).unwrap();
        for (index, (blue_x, blue_y)) in blue_centers.into_iter().enumerate() {
            let page = session.page(index as u32).unwrap();
            // Check the scale constructor directly; using only a fitted display
            // rectangle could hide incorrect native page-size scaling.
            for scale in [1.0, 2.0] {
                let transform = PageTransform::at_scale(
                    page.geometry,
                    crate::ViewportPoint { x: 37.0, y: 53.0 },
                    scale,
                )
                .unwrap();
                let viewport = transform
                    .pdf_to_viewport(PdfPoint { x: 75.0, y: 65.0 })
                    .unwrap();
                assert_eq!(viewport.x, 37.0 + blue_x * scale);
                assert_eq!(viewport.y, 53.0 + blue_y * scale);
                let pdf = transform.viewport_to_pdf(viewport).unwrap();
                assert_eq!(pdf, PdfPoint { x: 75.0, y: 65.0 });
            }
            for dpi in [72, 144, 150] {
                let mut png = Cursor::new(Vec::new());
                let rendered = crate::test_support::pdf_renderer()
                    .render_page_to_png_at_dpi(
                        PdfDpiRenderRequest {
                            source_path: session.source_path().to_path_buf(),
                            page_index: index as u32,
                            dpi,
                        },
                        &mut png,
                    )
                    .unwrap();
                assert_markers(
                    session,
                    index as u32,
                    rendered.width_pixels,
                    rendered.height_pixels,
                    png.get_ref(),
                );
                if dpi == 72 || dpi == 144 {
                    let transform = PageTransform::at_scale(
                        page.geometry,
                        crate::ViewportPoint { x: 0.0, y: 0.0 },
                        f64::from(dpi) / 72.0,
                    )
                    .unwrap();
                    assert_eq!(
                        transform.display_rect().width(),
                        f64::from(rendered.width_pixels)
                    );
                    assert_eq!(
                        transform.display_rect().height(),
                        f64::from(rendered.height_pixels)
                    );
                }
            }
            let mut png = Cursor::new(Vec::new());
            let rendered = crate::test_support::pdf_renderer()
                .render_page_to_png(
                    PdfRenderRequest {
                        source_path: session.source_path().to_path_buf(),
                        page_index: index as u32,
                        target_width: 401,
                    },
                    &mut png,
                )
                .unwrap();
            assert_markers(
                session,
                index as u32,
                rendered.width_pixels,
                rendered.height_pixels,
                png.get_ref(),
            );
        }
        assert_eq!(fs::read(session.source_path()).unwrap(), before);
    }
}

fn assert_markers(session: &EditorSession, index: u32, width: u32, height: u32, png: &[u8]) {
    let pixels = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .unwrap()
        .into_rgb8();
    assert_eq!(pixels.dimensions(), (width, height));
    let transform = PageTransform::new(
        session.page(index).unwrap().geometry,
        ViewportRect::new(37.0, 53.0, f64::from(width), f64::from(height)).unwrap(),
    )
    .unwrap();
    for (point, color) in [
        (PdfPoint { x: 75.0, y: 65.0 }, [0, 0, 255]),
        (PdfPoint { x: 175.0, y: 135.0 }, [255, 0, 0]),
    ] {
        let viewport = transform.pdf_to_viewport(point).unwrap();
        let x = (viewport.x - 37.0).floor() as u32;
        let y = (viewport.y - 53.0).floor() as u32;
        assert_eq!(
            pixels.get_pixel(x, y).0,
            color,
            "marker differs on page {index}"
        );
        let recovered = transform.viewport_to_pdf(viewport).unwrap();
        assert!((recovered.x - point.x).abs() < 1e-8);
        assert!((recovered.y - point.y).abs() < 1e-8);
    }
}

#[test]
fn rejects_invalid_empty_missing_and_unusable_native_documents() {
    for name in [
        "malformed.pdf",
        "editor_empty.pdf",
        "editor_disjoint_boxes.pdf",
    ] {
        assert_eq!(
            open_fixture(name)
                .err()
                .expect("invalid document rejected")
                .code,
            ApplicationErrorCode::InvalidPdf
        );
    }
    assert_eq!(
        open_fixture("missing-editor-fixture.pdf")
            .err()
            .unwrap()
            .code,
        ApplicationErrorCode::SourceNotFound
    );
}

#[test]
fn rejects_inconsistent_inspection_without_constructing_a_partial_session() {
    let mut document = crate::test_support::pdf_renderer()
        .inspect_page_geometry(&fixture("editor_geometry.pdf"))
        .unwrap();
    document.pages[5].display_width_points += 10.0;
    let error = open_with_inspector(Path::new("not-opened.pdf"), |_| Ok(document))
        .err()
        .unwrap();
    assert_eq!(error.code, ApplicationErrorCode::InvalidPdf);
}
