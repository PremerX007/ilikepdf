use super::*;
use crate::{PageBox, PageRotation};
use std::io::Cursor;
use std::path::Path;

fn pages() -> Vec<EditorPageMetadata> {
    [
        PageRotation::None,
        PageRotation::Clockwise90,
        PageRotation::HalfTurn,
        PageRotation::Clockwise270,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, rotation)| EditorPageMetadata {
        page_index: i as u32,
        geometry: PageGeometry::new(
            None,
            None,
            PageBox::new(25.0, 30.0, 225.0 + i as f64 * 20.0, 180.0).unwrap(),
            rotation,
        ),
    })
    .collect()
}
fn layout(scale: f64) -> EditorDocumentLayout {
    EditorDocumentLayout::from_pages(
        &pages(),
        800.0,
        EditorZoomMode::Custom(EditorZoom::new(scale).unwrap()),
    )
    .unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-7, "{a} != {b}");
}

#[test]
fn mixed_rotated_pages_share_their_exact_transform_rectangle() {
    for scale in [0.25, 0.64, 1.0, 2.5, 4.0] {
        let document = layout(scale);
        let mut top = 24.0;
        for page in document.pages() {
            let rect = page.rect();
            close(rect.top(), top);
            close(rect.left(), (document.width() - rect.width()) / 2.0);
            close(rect.width(), page.geometry.display_width_points() * scale);
            close(rect.height(), page.geometry.display_height_points() * scale);
            assert_eq!(rect, page.transform.display_rect());
            top += rect.height() + 24.0;
        }
        close(document.height(), top);
    }
}

#[test]
fn fit_width_uses_the_widest_rotated_page_and_clamps_limits() {
    for width in [100.0, 600.0, 5000.0] {
        let doc =
            EditorDocumentLayout::from_pages(&pages(), width, EditorZoomMode::FitWidth).unwrap();
        close(
            doc.zoom().scale(),
            ((width - 48.0) / 240.0).clamp(0.25, 4.0),
        );
        assert!(
            doc.pages()
                .iter()
                .all(|p| p.rect().width() <= doc.width() - 48.0 + 1e-8)
        );
    }
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            EditorDocumentLayout::from_pages(&pages(), invalid, EditorZoomMode::FitWidth).is_err()
        );
    }
    for invalid in [0.0, 0.24, 4.1, f64::NAN] {
        assert!(EditorZoom::new(invalid).is_err());
    }
    assert_eq!(EditorZoom::new(4.0).unwrap().stepped(true).scale(), 4.0);
    assert_eq!(EditorZoom::new(0.25).unwrap().stepped(false).scale(), 0.25);
}

#[test]
fn hit_testing_round_trips_corners_crop_origins_rotations_zoom_and_scroll() {
    for scale in [0.25, 0.64, 1.0, 2.5, 4.0] {
        let doc = layout(scale);
        for page in doc.pages() {
            let b = page.geometry.visible_box();
            for (x, y) in [
                (b.left_points(), b.bottom_points()),
                (b.right_points(), b.top_points()),
                (b.left_points(), b.top_points()),
                (b.right_points(), b.bottom_points()),
                (75.0, 65.0),
            ] {
                let document = page.transform.pdf_to_viewport(PdfPoint { x, y }).unwrap();
                let scroll = ViewportPoint {
                    x: 31.0,
                    y: page.rect().top() - 17.0,
                };
                let hit = doc
                    .hit_test(
                        ViewportPoint {
                            x: document.x - scroll.x,
                            y: document.y - scroll.y,
                        },
                        scroll,
                    )
                    .unwrap();
                assert_eq!(hit.page_index, page.page_index);
                close(hit.pdf_point.x, x);
                close(hit.pdf_point.y, y);
                close(hit.page_local_point.x, document.x - page.rect().left());
            }
        }
        let zero = ViewportPoint { x: 0.0, y: 0.0 };
        for point in [
            ViewportPoint { x: 400.0, y: 0.0 },
            ViewportPoint { x: -1.0, y: 50.0 },
            ViewportPoint {
                x: doc.width() + 1.0,
                y: 50.0,
            },
            ViewportPoint {
                x: 400.0,
                y: doc.height() + 1.0,
            },
            ViewportPoint {
                x: f64::NAN,
                y: 50.0,
            },
        ] {
            assert!(doc.hit_test(point, zero).is_none());
        }
        for page in &doc.pages()[..3] {
            assert!(
                doc.hit_test(
                    ViewportPoint {
                        x: doc.width() / 2.0,
                        y: page.rect().top() + page.rect().height() + 12.0
                    },
                    zero
                )
                .is_none()
            );
        }
    }
}

#[test]
fn center_source_anchor_survives_zoom_and_workspace_resize() {
    let old = layout(1.0);
    let page = old.pages()[2];
    let document = page
        .transform
        .pdf_to_viewport(PdfPoint { x: 90.0, y: 100.0 })
        .unwrap();
    let viewport = ViewportPoint { x: 180.0, y: 100.0 };
    let scroll = ViewportPoint {
        x: document.x - 90.0,
        y: document.y - 50.0,
    };
    for next in [
        layout(2.0),
        EditorDocumentLayout::from_pages(&pages(), 640.0, EditorZoomMode::FitWidth).unwrap(),
    ] {
        let anchored = next.anchored_scroll(&old, scroll, viewport).unwrap();
        let hit = next
            .hit_test(ViewportPoint { x: 90.0, y: 50.0 }, anchored)
            .unwrap();
        assert_eq!(hit.page_index, 2);
        close(hit.pdf_point.x, 90.0);
        close(hit.pdf_point.y, 100.0);
    }
    let gap_scroll = ViewportPoint {
        x: 0.0,
        y: old.pages()[0].rect().top() + old.pages()[0].rect().height() + 12.0 - 50.0,
    };
    assert!(
        layout(2.0)
            .anchored_scroll(&old, gap_scroll, viewport)
            .unwrap()
            .y
            .is_finite()
    );
}

#[test]
fn render_sizes_are_bounded_even_for_extreme_aspect_ratios() {
    for (w, h) in [(600.0, 800.0), (1e8, 1e8), (1.0, 1e8), (1e8, 1.0)] {
        let metadata = [EditorPageMetadata {
            page_index: 0,
            geometry: PageGeometry::new(
                None,
                None,
                PageBox::new(0.0, 0.0, w, h).unwrap(),
                PageRotation::None,
            ),
        }];
        let doc = EditorDocumentLayout::from_pages(
            &metadata,
            800.0,
            EditorZoomMode::Custom(EditorZoom::new(4.0).unwrap()),
        )
        .unwrap();
        for density in [1.0, 2.0, 8.0] {
            let size = doc.pages()[0].render_size(density).unwrap();
            assert!(size.width() <= EditorRenderSize::MAX_AXIS);
            assert!(size.height() <= EditorRenderSize::MAX_AXIS);
            assert!(
                u64::from(size.width()) * u64::from(size.height()) <= EditorRenderSize::MAX_PIXELS
            );
        }
    }
}

#[test]
fn desktop_rasters_cover_physical_display_through_400_percent_at_dpr_two() {
    for (w, h) in [(595.0, 842.0), (612.0, 792.0)] {
        for rotation in [
            PageRotation::None,
            PageRotation::Clockwise90,
            PageRotation::HalfTurn,
            PageRotation::Clockwise270,
        ] {
            let metadata = [EditorPageMetadata {
                page_index: 0,
                geometry: PageGeometry::new(
                    None,
                    None,
                    PageBox::new(25.0, 30.0, w + 25.0, h + 30.0).unwrap(),
                    rotation,
                ),
            }];
            for density in [1.0, 1.25, 1.5, 2.0] {
                let mut previous_pixels = 0;
                for scale in [1.0, 2.0, 3.0, 4.0] {
                    let doc = EditorDocumentLayout::from_pages(
                        &metadata,
                        900.0,
                        EditorZoomMode::Custom(EditorZoom::new(scale).unwrap()),
                    )
                    .unwrap();
                    let page = doc.pages()[0];
                    let before = page.transform;
                    let size = page.render_size(density).unwrap();
                    assert!(f64::from(size.width()) >= page.rect().width() * density);
                    assert!(f64::from(size.height()) >= page.rect().height() * density);
                    assert!(f64::from(size.width()) < page.rect().width() * density + 65.0);
                    assert!(f64::from(size.height()) < page.rect().height() * density + 65.0);
                    let pixels = u64::from(size.width()) * u64::from(size.height());
                    assert!(pixels > previous_pixels);
                    previous_pixels = pixels;
                    assert_eq!(page.transform, before);
                }
            }
        }
    }
    assert!(EditorRenderSize::new(8193, 1).is_err());
    assert!(EditorRenderSize::new(8192, 8192).is_err());
    assert!(EditorRenderSize::new(8192, 4096).is_ok());
    for density in [0.0, -1.0, 8.01, f64::INFINITY, f64::NAN] {
        assert!(layout(1.0).pages()[0].render_size(density).is_err());
    }
}

#[test]
fn insignificant_fit_width_changes_share_resolution_but_not_geometry() {
    let a = EditorDocumentLayout::from_pages(&pages(), 600.0, EditorZoomMode::FitWidth).unwrap();
    let b = EditorDocumentLayout::from_pages(&pages(), 600.01, EditorZoomMode::FitWidth).unwrap();
    assert_ne!(a.pages()[0].rect(), b.pages()[0].rect());
    for (a, b) in a.pages().iter().zip(b.pages()) {
        assert_eq!(a.render_size(1.5).unwrap(), b.render_size(1.5).unwrap());
    }
}

#[test]
fn native_markers_align_with_scrolled_multi_page_hit_testing() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ilikepdf_pdf/tests/fixtures/editor_geometry.pdf");
    let native = crate::test_support::pdf_renderer();
    let session =
        super::super::session::open_with_inspector(&path, |p| native.inspect_page_geometry(p))
            .unwrap();
    for scale in [0.5, 1.0, 2.0] {
        let doc = EditorDocumentLayout::new(
            &session,
            900.0,
            EditorZoomMode::Custom(EditorZoom::new(scale).unwrap()),
        )
        .unwrap();
        for page in &doc.pages()[2..] {
            let size = page.render_size(1.5).unwrap();
            let mut png = Cursor::new(Vec::new());
            native
                .render_page_raster(
                    ilikepdf_pdf::PdfPageRasterRequest {
                        source_path: path.clone(),
                        page_index: page.page_index,
                        width_pixels: size.width(),
                        height_pixels: size.height(),
                    },
                    &mut png,
                )
                .unwrap();
            let pixels = image::load_from_memory(png.get_ref()).unwrap().into_rgb8();
            assert_eq!(pixels.dimensions(), (size.width(), size.height()));
            for (point, color) in [
                (PdfPoint { x: 75.0, y: 65.0 }, [0, 0, 255]),
                (PdfPoint { x: 175.0, y: 135.0 }, [255, 0, 0]),
            ] {
                let document = page.transform.pdf_to_viewport(point).unwrap();
                let scroll = ViewportPoint {
                    x: 13.0,
                    y: page.rect().top() - 20.0,
                };
                let hit = doc
                    .hit_test(
                        ViewportPoint {
                            x: document.x - scroll.x,
                            y: document.y - scroll.y,
                        },
                        scroll,
                    )
                    .unwrap();
                assert_eq!(hit.page_index, page.page_index);
                close(hit.pdf_point.x, point.x);
                close(hit.pdf_point.y, point.y);
                let x = ((document.x - page.rect().left()) / page.rect().width()
                    * f64::from(size.width()))
                .floor() as u32;
                let y = ((document.y - page.rect().top()) / page.rect().height()
                    * f64::from(size.height()))
                .floor() as u32;
                assert_eq!(pixels.get_pixel(x, y).0, color);
            }
        }
    }
}

#[test]
fn long_document_layout_only_demands_visible_pages() {
    let mut metadata = Vec::new();
    for i in 0..1000 {
        let mut p = pages()[i % 4];
        p.page_index = i as u32;
        metadata.push(p);
    }
    let doc = EditorDocumentLayout::from_pages(
        &metadata,
        900.0,
        EditorZoomMode::Custom(EditorZoom::default()),
    )
    .unwrap();
    assert!(doc.visible_pages(0.0, 600.0, 120.0).len() < 8);
    let later = doc.visible_pages(doc.pages()[500].rect().top(), 600.0, 120.0);
    assert!(later.contains(&500));
    assert!(later.len() < 8);
    assert!(!later.contains(&0));
}
