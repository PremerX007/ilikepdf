use super::*;
use crate::application::editor::session::open_with_inspector;
use crate::{
    EditorDocumentLayout, EditorGestureKind, EditorResizeHandle, EditorZoom, EditorZoomMode,
    PageTransform, ViewportPoint,
};
use ilikepdf_pdf::{PdfDocumentGeometry, PdfPageBox, PdfPageGeometry, PdfPageRotation};
use std::path::Path;

fn session() -> EditorSession {
    open_with_inspector(Path::new("offline-fixture.pdf"), |_| {
        Ok(PdfDocumentGeometry {
            pages: [
                PdfPageRotation::None,
                PdfPageRotation::Clockwise90,
                PdfPageRotation::HalfTurn,
                PdfPageRotation::Clockwise270,
            ]
            .into_iter()
            .enumerate()
            .map(|(i, rotation)| {
                let width = 200.0 + i as f64 * 20.0;
                let rotated = matches!(
                    rotation,
                    PdfPageRotation::Clockwise90 | PdfPageRotation::Clockwise270
                );
                PdfPageGeometry {
                    declared_media_box: None,
                    declared_crop_box: None,
                    visible_box: PdfPageBox {
                        left_points: 25.0,
                        bottom_points: -30.0,
                        right_points: 25.0 + width,
                        top_points: 120.0,
                    },
                    rotation,
                    display_width_points: if rotated { 150.0 } else { width },
                    display_height_points: if rotated { width } else { 150.0 },
                }
            })
            .collect(),
        })
    })
    .unwrap()
}
fn rect() -> EditorRectangle {
    EditorRectangle::new(75., 10., 125., 50.).unwrap()
}
fn layout(session: &EditorSession, scale: f64) -> EditorDocumentLayout {
    EditorDocumentLayout::new(
        session,
        800.,
        EditorZoomMode::Custom(EditorZoom::new(scale).unwrap()),
    )
    .unwrap()
}
fn point(transform: PageTransform, x: f64, y: f64) -> ViewportPoint {
    transform.pdf_to_viewport(PdfPoint { x, y }).unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-7, "{a} != {b}");
}

#[test]
fn validates_geometry_page_ownership_and_minimum() {
    let mut edits = EditorEditState::new(&session());
    for bounds in [
        (0., 0., 0., 1.),
        (0., 0., 1., -1.),
        (f64::NAN, 0., 1., 1.),
        (0., 0., f64::INFINITY, 1.),
        (-f64::MAX, 0., f64::MAX, 1.),
    ] {
        assert_eq!(
            EditorRectangle::new(bounds.0, bounds.1, bounds.2, bounds.3),
            Err(EditorEditError::InvalidRectangle)
        );
    }
    assert_eq!(
        edits.insert_prototype(4, rect()),
        Err(EditorEditError::InvalidPage)
    );
    for r in [
        EditorRectangle::new(0., 0., 100., 100.).unwrap(),
        EditorRectangle::new(25., 0., 26., 1.).unwrap(),
    ] {
        assert_eq!(
            edits.insert_prototype(0, r),
            Err(EditorEditError::InvalidRectangle)
        );
    }
    assert_eq!(edits.undo_count(), 0);
    for page in 0..4 {
        edits.add_prototype(page).unwrap();
    }
    assert_eq!(
        edits
            .objects()
            .iter()
            .map(|o| o.page_index())
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
}

#[test]
fn stable_ids_stacking_delete_restore_and_redo_branch() {
    let mut edits = EditorEditState::new(&session());
    let a = edits.insert_prototype(0, rect()).unwrap();
    let b = edits.insert_prototype(0, rect()).unwrap();
    assert_ne!(a, b);
    assert_eq!(edits.select_at(0, PdfPoint { x: 100., y: 30. }), Some(b));
    edits.delete_selected();
    assert_eq!(edits.selected(), None);
    assert_eq!(edits.select_at(0, PdfPoint { x: 100., y: 30. }), Some(a));
    edits.undo();
    assert_eq!(edits.objects()[1].id(), b);
    assert_eq!(edits.objects()[1].rectangle(), rect());
    assert_eq!(edits.select_at(0, PdfPoint { x: 100., y: 30. }), Some(b));
    edits.redo();
    assert_eq!(edits.objects().len(), 1);
    edits.undo();
    let c = edits.insert_prototype(1, rect()).unwrap();
    assert!(c.value() > b.value());
    assert_eq!(edits.redo_count(), 0);
    assert!(!edits.redo());
    assert_eq!(edits.select_at(2, PdfPoint { x: 100., y: 30. }), None);
}

#[test]
fn sequential_add_move_resize_delete_undo_and_redo() {
    let session = session();
    let transform = layout(&session, 1.).pages()[0].transform;
    let mut edits = EditorEditState::new(&session);
    let a = edits.insert_prototype(0, rect()).unwrap();
    let gesture = edits
        .begin_gesture(0, transform, point(transform, 100., 30.))
        .unwrap()
        .unwrap();
    let moved_point = point(transform, 110., 40.);
    for _ in 0..80 {
        gesture.preview(moved_point).unwrap();
    }
    assert_eq!(edits.undo_count(), 1);
    assert_eq!(edits.objects()[0].rectangle(), rect());
    edits.finish_gesture(&gesture, moved_point).unwrap();
    let moved = edits.objects()[0].rectangle();
    assert_eq!(moved, EditorRectangle::new(85., 20., 135., 60.).unwrap());
    let handle = EditorResizeHandle::TopRight.point(moved.display_rect(transform).unwrap());
    let gesture = edits.begin_gesture(0, transform, handle).unwrap().unwrap();
    assert_eq!(
        gesture.kind(),
        EditorGestureKind::Resize(EditorResizeHandle::TopRight)
    );
    let end = ViewportPoint {
        x: handle.x + 10.,
        y: handle.y - 10.,
    };
    for _ in 0..80 {
        gesture.preview(end).unwrap();
    }
    assert_eq!(edits.undo_count(), 2);
    edits.finish_gesture(&gesture, end).unwrap();
    let resized = edits.objects()[0].rectangle();
    assert_eq!(resized, EditorRectangle::new(85., 20., 145., 70.).unwrap());
    edits.delete_selected();
    assert_eq!(edits.undo_count(), 4);
    edits.undo();
    assert_eq!(edits.objects()[0].id(), a);
    assert_eq!(edits.objects()[0].rectangle(), resized);
    edits.undo();
    assert_eq!(edits.objects()[0].rectangle(), moved);
    edits.undo();
    assert_eq!(edits.objects()[0].rectangle(), rect());
    edits.undo();
    assert!(edits.objects().is_empty());
    edits.redo();
    assert_eq!(edits.objects()[0].id(), a);
    edits.redo();
    assert_eq!(edits.objects()[0].rectangle(), moved);
    edits.redo();
    assert_eq!(edits.objects()[0].rectangle(), resized);
    edits.redo();
    assert!(edits.objects().is_empty());
}

#[test]
fn bounded_history_and_objects_and_noop_gestures() {
    let session = session();
    let mut edits = EditorEditState::new(&session);
    let transform = layout(&session, 1.).pages()[0].transform;
    edits.insert_prototype(0, rect()).unwrap();
    let p = point(transform, 100., 30.);
    let gesture = edits.begin_gesture(0, transform, p).unwrap().unwrap();
    assert!(!edits.finish_gesture(&gesture, p).unwrap());
    assert_eq!(edits.undo_count(), 1);
    for _ in 1..EDITOR_OBJECT_LIMIT {
        edits.insert_prototype(0, rect()).unwrap();
    }
    assert_eq!(edits.undo_count(), EDITOR_HISTORY_LIMIT);
    assert_eq!(
        edits.add_prototype(0),
        Err(EditorEditError::CapacityReached)
    );
    for _ in 0..EDITOR_HISTORY_LIMIT {
        assert!(edits.undo());
    }
    assert!(!edits.undo());
    assert_eq!(
        edits.objects().len(),
        EDITOR_OBJECT_LIMIT - EDITOR_HISTORY_LIMIT
    );
    for _ in 0..EDITOR_HISTORY_LIMIT {
        assert!(edits.redo());
    }
    assert_eq!(edits.undo_count(), EDITOR_HISTORY_LIMIT);
}

#[test]
fn all_rotations_zoom_crop_scroll_and_raster_independence() {
    let session = session();
    for scale in [0.25, 1., 2., 4., 1.] {
        let layout = layout(&session, scale);
        for page in layout.pages() {
            let mut edits = EditorEditState::new(&session);
            edits.insert_prototype(page.page_index, rect()).unwrap();
            let start = point(page.transform, 100., 30.);
            let end = point(page.transform, 111., 37.);
            let scroll = ViewportPoint {
                x: 31.,
                y: page.rect().top() - 17.,
            };
            let hit = layout
                .hit_test(
                    ViewportPoint {
                        x: start.x - scroll.x,
                        y: start.y - scroll.y,
                    },
                    scroll,
                )
                .unwrap();
            assert_eq!(hit.page_index, page.page_index);
            close(hit.pdf_point.x, 100.);
            close(hit.pdf_point.y, 30.);
            let g = edits
                .begin_gesture(page.page_index, page.transform, start)
                .unwrap()
                .unwrap();
            edits.finish_gesture(&g, end).unwrap();
            let b = edits.objects()[0].rectangle().bounds();
            close(b.left_points(), 86.);
            close(b.bottom_points(), 17.);
            let before = edits.objects().to_vec();
            let history = edits.undo_count();
            for density in [1., 2., 3.] {
                page.render_size(density).unwrap();
            }
            edits.objects()[0]
                .rectangle()
                .display_rect(page.transform)
                .unwrap();
            assert_eq!(edits.objects(), before);
            assert_eq!(edits.undo_count(), history);
            edits.undo();
            assert_eq!(edits.objects()[0].rectangle(), rect());
            edits.redo();
            assert_eq!(edits.objects(), before);
        }
    }
}

#[test]
fn eight_handles_keep_opposite_edges_stable_and_clamp_at_every_rotation() {
    let session = session();
    for scale in [0.25, 1., 4.] {
        for page in layout(&session, scale).pages() {
            for handle in EditorResizeHandle::ALL {
                let mut edits = EditorEditState::new(&session);
                edits.insert_prototype(page.page_index, rect()).unwrap();
                let display = rect().display_rect(page.transform).unwrap();
                let start = handle.point(display);
                let g = edits
                    .begin_gesture(page.page_index, page.transform, start)
                    .unwrap()
                    .unwrap();
                assert_eq!(g.kind(), EditorGestureKind::Resize(handle));
                let source_handle = page.transform.viewport_to_pdf(start).unwrap();
                for end in [
                    ViewportPoint {
                        x: start.x + 7.,
                        y: start.y + 9.,
                    },
                    ViewportPoint {
                        x: -100000.,
                        y: -100000.,
                    },
                    ViewportPoint {
                        x: 100000.,
                        y: 100000.,
                    },
                ] {
                    let result = g.preview(end).unwrap();
                    result.validate_on(page.geometry.visible_box()).unwrap();
                    let b = result.bounds();
                    let old = rect().bounds();
                    if (source_handle.x - old.left_points()).abs() > 1e-7 {
                        close(b.left_points(), old.left_points());
                    }
                    if (source_handle.x - old.right_points()).abs() > 1e-7 {
                        close(b.right_points(), old.right_points());
                    }
                    if (source_handle.y - old.bottom_points()).abs() > 1e-7 {
                        close(b.bottom_points(), old.bottom_points());
                    }
                    if (source_handle.y - old.top_points()).abs() > 1e-7 {
                        close(b.top_points(), old.top_points());
                    }
                }
            }
        }
    }
}

#[test]
fn session_replacement_and_intervening_edits_reject_stale_transactions() {
    let session = session();
    let mut edits = EditorEditState::new(&session);
    edits.insert_prototype(0, rect()).unwrap();
    let transform = layout(&session, 1.).pages()[0].transform;
    let p = point(transform, 100., 30.);
    let g = edits.begin_gesture(0, transform, p).unwrap().unwrap();
    edits.delete_selected();
    assert_eq!(
        edits.finish_gesture(&g, p),
        Err(EditorEditError::StaleGesture)
    );
    let mut replacement = EditorEditState::new(&self::session());
    assert!(replacement.objects().is_empty());
    assert_eq!(replacement.selected(), None);
    assert_eq!(replacement.undo_count(), 0);
    assert_eq!(replacement.redo_count(), 0);
    assert_eq!(
        replacement.finish_gesture(&g, p),
        Err(EditorEditError::StaleGesture)
    );
}

#[test]
fn fractional_source_rectangles_clamp_at_page_edges_and_minimum_without_rounding_failures() {
    let session = session();
    for scale in [0.25, 0.64, 1.25, 4.0] {
        for page in layout(&session, scale).pages() {
            let mut edits = EditorEditState::new(&session);
            let rectangle =
                EditorRectangle::new(75.123456789, 10.234567891, 125.876543219, 50.345678912)
                    .unwrap();
            edits.insert_prototype(page.page_index, rectangle).unwrap();
            let start = point(page.transform, 100.0, 30.0);
            let move_gesture = edits
                .begin_gesture(page.page_index, page.transform, start)
                .unwrap()
                .unwrap();
            for end in [
                ViewportPoint { x: -1e6, y: -1e6 },
                ViewportPoint { x: 1e6, y: 1e6 },
            ] {
                let result = move_gesture.preview(end).unwrap();
                result.validate_on(page.geometry.visible_box()).unwrap();
                close(
                    result.bounds().width_points(),
                    rectangle.bounds().width_points(),
                );
                close(
                    result.bounds().height_points(),
                    rectangle.bounds().height_points(),
                );
            }
            for handle in EditorResizeHandle::ALL {
                let start = handle.point(rectangle.display_rect(page.transform).unwrap());
                let resize = edits
                    .begin_gesture(page.page_index, page.transform, start)
                    .unwrap()
                    .unwrap();
                for end in [
                    ViewportPoint { x: -1e6, y: -1e6 },
                    ViewportPoint { x: 1e6, y: 1e6 },
                ] {
                    resize
                        .preview(end)
                        .unwrap()
                        .validate_on(page.geometry.visible_box())
                        .unwrap();
                }
            }
        }
    }
}
