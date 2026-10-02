use super::*;

#[test]
fn validates_boxes_and_preserves_negative_origins() {
    let page = PageBox::new(-100.0, -200.0, 200.0, 300.0).unwrap();
    assert_eq!((page.width_points(), page.height_points()), (300.0, 500.0));
    assert_eq!((page.left_points(), page.bottom_points()), (-100.0, -200.0));
    for values in [
        (0.0, 0.0, 0.0, 100.0),
        (0.0, 0.0, 100.0, 0.0),
        (10.0, 0.0, 0.0, 10.0),
        (0.0, 10.0, 10.0, 0.0),
        (f64::NAN, 0.0, 100.0, 100.0),
        (0.0, 0.0, f64::INFINITY, 100.0),
        (-f64::MAX, 0.0, f64::MAX, 100.0),
    ] {
        assert_eq!(
            PageBox::new(values.0, values.1, values.2, values.3),
            Err(PageGeometryError::InvalidPageBox)
        );
    }
}

#[test]
fn visible_dimensions_and_intrinsic_rotation_are_distinct_from_media_size() {
    let media = PageBox::new(-50.0, -40.0, 350.0, 260.0).unwrap();
    let crop = PageBox::new(25.0, 30.0, 225.0, 180.0).unwrap();
    for (rotation, display) in [
        (PageRotation::None, (200.0, 150.0)),
        (PageRotation::Clockwise90, (150.0, 200.0)),
        (PageRotation::HalfTurn, (200.0, 150.0)),
        (PageRotation::Clockwise270, (150.0, 200.0)),
    ] {
        let geometry = PageGeometry::new(Some(media), Some(crop), crop, rotation);
        assert_eq!(geometry.declared_media_box(), Some(media));
        assert_eq!(geometry.declared_crop_box(), Some(crop));
        assert_eq!(
            (geometry.width_points(), geometry.height_points()),
            (200.0, 150.0)
        );
        assert_eq!(
            (
                geometry.display_width_points(),
                geometry.display_height_points()
            ),
            display
        );
    }
}
