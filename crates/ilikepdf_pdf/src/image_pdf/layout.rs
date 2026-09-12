use super::{ImagePdfLayout, ImagePdfOrientation, ImagePdfPageInfo, ImagePdfPageSize};
use crate::{PdfError, PdfErrorKind};

const POINTS_PER_INCH: f32 = 72.0;
const MILLIMETERS_PER_INCH: f32 = 25.4;
const FIT_PIXELS_PER_INCH: f32 = 96.0;
const A4_WIDTH_MM: f32 = 210.0;
const A4_HEIGHT_MM: f32 = 297.0;
const LETTER_WIDTH_MM: f32 = 215.9;
const LETTER_HEIGHT_MM: f32 = 279.4;

pub(super) fn calculate_page_layout(
    pixel_width: u32,
    pixel_height: u32,
    layout: ImagePdfLayout,
) -> Result<ImagePdfPageInfo, PdfError> {
    let (page_width, page_height, margin) = match layout.page_size {
        ImagePdfPageSize::Fit => (
            pixel_width as f32 / FIT_PIXELS_PER_INCH * POINTS_PER_INCH,
            pixel_height as f32 / FIT_PIXELS_PER_INCH * POINTS_PER_INCH,
            0.0,
        ),
        ImagePdfPageSize::A4 => standard_page_size(A4_WIDTH_MM, A4_HEIGHT_MM, layout),
        ImagePdfPageSize::UsLetter => standard_page_size(LETTER_WIDTH_MM, LETTER_HEIGHT_MM, layout),
    };
    let available_width = page_width - margin * 2.0;
    let available_height = page_height - margin * 2.0;
    if available_width <= 0.0 || available_height <= 0.0 {
        return Err(PdfError::new(PdfErrorKind::ImagePlacementFailed));
    }

    let width_scale = available_width / pixel_width as f32;
    let height_scale = available_height / pixel_height as f32;
    let scale = width_scale.min(height_scale);
    let image_width = pixel_width as f32 * scale;
    let image_height = pixel_height as f32 * scale;

    Ok(ImagePdfPageInfo {
        width_points: page_width,
        height_points: page_height,
        image_left_points: margin + (available_width - image_width) / 2.0,
        image_bottom_points: margin + (available_height - image_height) / 2.0,
        image_width_points: image_width,
        image_height_points: image_height,
    })
}

fn standard_page_size(
    portrait_width_mm: f32,
    portrait_height_mm: f32,
    layout: ImagePdfLayout,
) -> (f32, f32, f32) {
    let width = millimeters_to_points(portrait_width_mm);
    let height = millimeters_to_points(portrait_height_mm);
    let margin = millimeters_to_points(layout.margin.millimeters());
    match layout.orientation {
        ImagePdfOrientation::Portrait => (width, height, margin),
        ImagePdfOrientation::Landscape => (height, width, margin),
    }
}

fn millimeters_to_points(millimeters: f32) -> f32 {
    millimeters / MILLIMETERS_PER_INCH * POINTS_PER_INCH
}

#[cfg(test)]
mod tests;
