use super::*;
use crate::ImagePdfMargin;

#[test]
fn fit_uses_96_ppi_and_ignores_orientation_and_margin() {
    let layout = calculate_page_layout(
        960,
        480,
        ImagePdfLayout {
            page_size: ImagePdfPageSize::Fit,
            orientation: ImagePdfOrientation::Landscape,
            margin: ImagePdfMargin::Big,
        },
    )
    .expect("fit layout should be valid");

    assert_eq!(layout.width_points, 720.0);
    assert_eq!(layout.height_points, 360.0);
    assert_eq!(layout.image_left_points, 0.0);
    assert_eq!(layout.image_bottom_points, 0.0);
    assert_eq!(layout.image_width_points, 720.0);
    assert_eq!(layout.image_height_points, 360.0);
}

#[test]
fn standard_page_centers_without_stretching() {
    let layout = calculate_page_layout(
        400,
        200,
        ImagePdfLayout {
            page_size: ImagePdfPageSize::A4,
            orientation: ImagePdfOrientation::Portrait,
            margin: ImagePdfMargin::Small,
        },
    )
    .expect("A4 layout should be valid");

    assert!((layout.width_points - 595.2756).abs() < 0.001);
    assert!((layout.height_points - 841.8898).abs() < 0.001);
    assert!((layout.image_width_points / layout.image_height_points - 2.0).abs() < 0.001);
    assert!((layout.image_left_points - 28.346_457).abs() < 0.001);
    assert!(layout.image_bottom_points > 28.346_457);
}
