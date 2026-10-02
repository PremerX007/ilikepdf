use super::*;
use crate::PageBox;

fn geometry(
    left: f64,
    bottom: f64,
    width: f64,
    height: f64,
    rotation: PageRotation,
) -> PageGeometry {
    let rect = PageBox::new(left, bottom, left + width, bottom + height).unwrap();
    PageGeometry::new(Some(rect), None, rect, rotation)
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-8, "{actual} != {expected}");
}

#[test]
fn maps_all_four_source_corners_to_independently_expected_display_corners() {
    // Source corner order: bottom-left, bottom-right, top-left, top-right.
    for (rotation, expected) in [
        (
            PageRotation::None,
            [(0.0, 150.0), (200.0, 150.0), (0.0, 0.0), (200.0, 0.0)],
        ),
        (
            PageRotation::Clockwise90,
            [(0.0, 0.0), (0.0, 200.0), (150.0, 0.0), (150.0, 200.0)],
        ),
        (
            PageRotation::HalfTurn,
            [(200.0, 0.0), (0.0, 0.0), (200.0, 150.0), (0.0, 150.0)],
        ),
        (
            PageRotation::Clockwise270,
            [(150.0, 200.0), (150.0, 0.0), (0.0, 200.0), (0.0, 0.0)],
        ),
    ] {
        let transform = PageTransform::at_scale(
            geometry(25.0, 30.0, 200.0, 150.0, rotation),
            ViewportPoint { x: 17.0, y: -41.0 },
            2.5,
        )
        .unwrap();
        for ((x, y), (a, b)) in [(25.0, 30.0), (225.0, 30.0), (25.0, 180.0), (225.0, 180.0)]
            .into_iter()
            .zip(expected)
        {
            let viewport = transform.pdf_to_viewport(PdfPoint { x, y }).unwrap();
            close(viewport.x, 17.0 + a * 2.5);
            close(viewport.y, -41.0 + b * 2.5);
            let pdf = transform
                .viewport_to_pdf(ViewportPoint {
                    x: 17.0 + a * 2.5,
                    y: -41.0 + b * 2.5,
                })
                .unwrap();
            close(pdf.x, x);
            close(pdf.y, y);
        }
    }
}

#[test]
fn round_trips_corners_centers_and_arbitrary_points_across_sizes_origins_and_scales() {
    for rotation in [
        PageRotation::None,
        PageRotation::Clockwise90,
        PageRotation::HalfTurn,
        PageRotation::Clockwise270,
    ] {
        for (left, bottom, width, height) in [
            (0.0, 0.0, 595.2756, 841.8898),
            (0.0, 0.0, 612.0, 792.0),
            (25.0, 30.0, 200.0, 150.0),
            (-100.0, -200.0, 300.0, 500.0),
            (123.25, -42.75, 1000.0, 20.0),
        ] {
            let geometry = geometry(left, bottom, width, height, rotation);
            for scale in [0.125, 0.5, 1.0, 1.375, 2.0, 4.25] {
                let transform = PageTransform::at_scale(
                    geometry,
                    ViewportPoint {
                        x: -87.25,
                        y: 135.5,
                    },
                    scale,
                )
                .unwrap();
                for (fx, fy) in [
                    (0.0, 0.0),
                    (1.0, 0.0),
                    (0.0, 1.0),
                    (1.0, 1.0),
                    (0.5, 0.5),
                    (0.137, 0.719),
                    (0.823, 0.061),
                ] {
                    let original = PdfPoint {
                        x: left + width * fx,
                        y: bottom + height * fy,
                    };
                    let viewport = transform.pdf_to_viewport(original).unwrap();
                    let recovered = transform.viewport_to_pdf(viewport).unwrap();
                    close(recovered.x, original.x);
                    close(recovered.y, original.y);
                    let viewport_again = transform.pdf_to_viewport(recovered).unwrap();
                    close(viewport_again.x, viewport.x);
                    close(viewport_again.y, viewport.y);
                }
            }
        }
    }
}

#[test]
fn uses_actual_display_rectangle_with_independent_axis_scaling() {
    let geometry = geometry(25.0, 30.0, 200.0, 150.0, PageRotation::Clockwise90);
    let rect = ViewportRect::new(50.0, 70.0, 313.0, 417.0).unwrap();
    let transform = PageTransform::new(geometry, rect).unwrap();
    assert_eq!(transform.geometry(), geometry);
    assert_eq!(transform.display_rect(), rect);
    assert_eq!(
        (rect.left(), rect.top(), rect.width(), rect.height()),
        (50.0, 70.0, 313.0, 417.0)
    );
    close(transform.scale_x(), 313.0 / 150.0);
    close(transform.scale_y(), 417.0 / 200.0);
    let viewport = transform
        .pdf_to_viewport(PdfPoint { x: 75.0, y: 65.0 })
        .unwrap();
    close(viewport.x, 50.0 + 313.0 * 35.0 / 150.0);
    close(viewport.y, 70.0 + 417.0 * 50.0 / 200.0);
    let pdf = transform.viewport_to_pdf(viewport).unwrap();
    close(pdf.x, 75.0);
    close(pdf.y, 65.0);
}

#[test]
fn rejects_nonfinite_or_off_page_points_in_both_directions() {
    for rotation in [
        PageRotation::None,
        PageRotation::Clockwise90,
        PageRotation::HalfTurn,
        PageRotation::Clockwise270,
    ] {
        let geometry = geometry(25.0, 30.0, 200.0, 150.0, rotation);
        let transform =
            PageTransform::at_scale(geometry, ViewportPoint { x: 10.0, y: 20.0 }, 1.5).unwrap();
        for (x, y) in [(24.0, 35.0), (226.0, 35.0), (30.0, 29.0), (30.0, 181.0)] {
            assert_eq!(
                transform.pdf_to_viewport(PdfPoint { x, y }),
                Err(PageGeometryError::PointOutsidePage)
            );
        }
        let rect = transform.display_rect();
        for (x, y) in [
            (9.0, 20.0),
            (10.0 + rect.width() + 1.0, 20.0),
            (10.0, 19.0),
            (10.0, 20.0 + rect.height() + 1.0),
        ] {
            assert_eq!(
                transform.viewport_to_pdf(ViewportPoint { x, y }),
                Err(PageGeometryError::PointOutsideViewport)
            );
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                transform.pdf_to_viewport(PdfPoint { x: value, y: 35.0 }),
                Err(PageGeometryError::NonFinitePoint)
            );
            assert_eq!(
                transform.pdf_to_viewport(PdfPoint { x: 30.0, y: value }),
                Err(PageGeometryError::NonFinitePoint)
            );
            assert_eq!(
                transform.viewport_to_pdf(ViewportPoint { x: value, y: 20.0 }),
                Err(PageGeometryError::NonFinitePoint)
            );
            assert_eq!(
                transform.viewport_to_pdf(ViewportPoint { x: 10.0, y: value }),
                Err(PageGeometryError::NonFinitePoint)
            );
        }
    }
}

#[test]
fn accepts_only_numerical_boundary_drift_without_clamping_real_outside_points() {
    let transform = PageTransform::at_scale(
        geometry(25.0, 30.0, 200.0, 150.0, PageRotation::None),
        ViewportPoint { x: 0.0, y: 0.0 },
        1.0,
    )
    .unwrap();
    assert!(
        transform
            .pdf_to_viewport(PdfPoint {
                x: 25.0 - 1e-9,
                y: 30.0
            })
            .is_ok()
    );
    assert_eq!(
        transform.pdf_to_viewport(PdfPoint {
            x: 25.0 - 1e-5,
            y: 30.0
        }),
        Err(PageGeometryError::PointOutsidePage)
    );
    assert!(
        transform
            .viewport_to_pdf(ViewportPoint { x: -1e-9, y: 0.0 })
            .is_ok()
    );
    assert_eq!(
        transform.viewport_to_pdf(ViewportPoint { x: -1e-5, y: 0.0 }),
        Err(PageGeometryError::PointOutsideViewport)
    );
}

#[test]
fn rejects_invalid_display_rectangles_and_scales() {
    for (left, top, width, height) in [
        (0.0, 0.0, 0.0, 100.0),
        (0.0, 0.0, 100.0, -1.0),
        (f64::NAN, 0.0, 100.0, 100.0),
        (0.0, f64::INFINITY, 100.0, 100.0),
        (0.0, 0.0, f64::INFINITY, 100.0),
        (f64::MAX, 0.0, f64::MAX, 100.0),
        (1e30, 0.0, 1.0, 100.0),
    ] {
        assert_eq!(
            ViewportRect::new(left, top, width, height),
            Err(PageGeometryError::InvalidDisplayRectangle)
        );
    }
    let geometry = geometry(0.0, 0.0, 300.0, 200.0, PageRotation::None);
    for scale in [
        0.0,
        -1.0,
        f64::NAN,
        f64::INFINITY,
        f64::MAX,
        f64::from_bits(1),
    ] {
        assert_eq!(
            PageTransform::at_scale(geometry, ViewportPoint { x: 0.0, y: 0.0 }, scale),
            Err(PageGeometryError::InvalidDisplayRectangle)
        );
    }
}
