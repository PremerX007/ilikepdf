use std::fs;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use image::{codecs::png::PngDecoder, codecs::webp::WebPDecoder};
use pdfium_render::prelude::{
    PdfPageImageObject, PdfPageObjectsCommon, PdfPagePaperSize, PdfPoints, Pdfium,
};

use crate::{PdfError, PdfErrorKind};

const POINTS_PER_INCH: f32 = 72.0;
const MILLIMETERS_PER_INCH: f32 = 25.4;
const FIT_PIXELS_PER_INCH: f32 = 96.0;
const A4_WIDTH_MM: f32 = 210.0;
const A4_HEIGHT_MM: f32 = 297.0;
const LETTER_WIDTH_MM: f32 = 215.9;
const LETTER_HEIGHT_MM: f32 = 279.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePdfPageSize {
    Fit,
    A4,
    UsLetter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePdfOrientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePdfMargin {
    None,
    Small,
    Big,
}

impl ImagePdfMargin {
    const fn millimeters(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Small => 10.0,
            Self::Big => 20.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImagePdfLayout {
    pub page_size: ImagePdfPageSize,
    pub orientation: ImagePdfOrientation,
    pub margin: ImagePdfMargin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePdfRequest {
    pub source_paths: Vec<PathBuf>,
    pub layout: ImagePdfLayout,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImagePdfPageInfo {
    pub width_points: f32,
    pub height_points: f32,
    pub image_left_points: f32,
    pub image_bottom_points: f32,
    pub image_width_points: f32,
    pub image_height_points: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImagePdfResult {
    pub page_count: u32,
    pub pages: Vec<ImagePdfPageInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    pub width_pixels: u32,
    pub height_pixels: u32,
}

pub(crate) fn create_image_pdf<W: Write + 'static>(
    pdfium: &Pdfium,
    request: ImagePdfRequest,
    output: &mut W,
    mut on_page_complete: impl FnMut(ImagePdfPageInfo),
) -> Result<ImagePdfResult, PdfError> {
    if request.source_paths.is_empty() {
        return Err(PdfError::new(PdfErrorKind::DocumentCreateFailed));
    }

    let mut document = pdfium
        .create_new_pdf()
        .map_err(|_| PdfError::new(PdfErrorKind::DocumentCreateFailed))?;
    let mut page_infos = Vec::with_capacity(request.source_paths.len());

    for source_path in &request.source_paths {
        let decoded = decode_visually_oriented_image(source_path)?;
        let page_info = calculate_page_layout(
            decoded.image.width(),
            decoded.image.height(),
            request.layout,
        )?;
        let page_size = PdfPagePaperSize::new_custom(
            PdfPoints::new(page_info.width_points),
            PdfPoints::new(page_info.height_points),
        );
        let mut page = document
            .pages_mut()
            .create_page_at_end(page_size)
            .map_err(|_| PdfError::new(PdfErrorKind::PageCreateFailed))?;
        if decoded.can_embed_source_jpeg {
            let mut image_object =
                PdfPageImageObject::new_from_jpeg_file(&document, source_path)
                    .map_err(|_| PdfError::new(PdfErrorKind::ImagePlacementFailed))?;
            image_object
                .scale(page_info.image_width_points, page_info.image_height_points)
                .and_then(|_| {
                    image_object.translate(
                        PdfPoints::new(page_info.image_left_points),
                        PdfPoints::new(page_info.image_bottom_points),
                    )
                })
                .map_err(|_| PdfError::new(PdfErrorKind::ImagePlacementFailed))?;
            page.objects_mut()
                .add_image_object(image_object)
                .map_err(|_| PdfError::new(PdfErrorKind::ImagePlacementFailed))?;
        } else {
            page.objects_mut()
                .create_image_object(
                    PdfPoints::new(page_info.image_left_points),
                    PdfPoints::new(page_info.image_bottom_points),
                    &decoded.image,
                    Some(PdfPoints::new(page_info.image_width_points)),
                    Some(PdfPoints::new(page_info.image_height_points)),
                )
                .map_err(|_| PdfError::new(PdfErrorKind::ImagePlacementFailed))?;
        }
        drop(page);
        drop(decoded);

        on_page_complete(page_info.clone());
        page_infos.push(page_info);
    }

    document
        .save_to_writer(output)
        .map_err(|_| PdfError::new(PdfErrorKind::SaveFailed))?;

    Ok(ImagePdfResult {
        page_count: u32::try_from(page_infos.len())
            .map_err(|_| PdfError::new(PdfErrorKind::DocumentCreateFailed))?,
        pages: page_infos,
    })
}

struct DecodedImage {
    image: DynamicImage,
    can_embed_source_jpeg: bool,
}

fn decode_visually_oriented_image(source_path: &Path) -> Result<DecodedImage, PdfError> {
    validate_source(source_path)?;
    validate_extension(source_path)?;

    let reader = ImageReader::open(source_path)
        .map_err(map_source_io_error)?
        .with_guessed_format()
        .map_err(map_source_io_error)?;
    let format = reader
        .format()
        .ok_or_else(|| PdfError::new(PdfErrorKind::UnsupportedImageFormat))?;
    if !matches!(
        format,
        ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP
    ) {
        return Err(PdfError::new(PdfErrorKind::UnsupportedImageFormat));
    }
    reject_animated_input(source_path, format)?;

    let mut decoder = reader
        .into_decoder()
        .map_err(|_| PdfError::new(PdfErrorKind::MalformedImage))?;
    let orientation = decoder
        .orientation()
        .map_err(|_| PdfError::new(PdfErrorKind::ImageOrientationFailed))?;
    let mut decoded = DynamicImage::from_decoder(decoder)
        .map_err(|_| PdfError::new(PdfErrorKind::ImageDecodeFailed))?;
    decoded.apply_orientation(orientation);

    if decoded.width() == 0 || decoded.height() == 0 {
        return Err(PdfError::new(PdfErrorKind::MalformedImage));
    }

    Ok(DecodedImage {
        image: decoded,
        can_embed_source_jpeg: format == ImageFormat::Jpeg
            && orientation == Orientation::NoTransforms,
    })
}

fn reject_animated_input(source_path: &Path, format: ImageFormat) -> Result<(), PdfError> {
    let is_animated = match format {
        ImageFormat::Png => PngDecoder::new(BufReader::new(
            fs::File::open(source_path).map_err(map_source_io_error)?,
        ))
        .and_then(|decoder| decoder.is_apng())
        .map_err(|_| PdfError::new(PdfErrorKind::MalformedImage))?,
        ImageFormat::WebP => WebPDecoder::new(BufReader::new(
            fs::File::open(source_path).map_err(map_source_io_error)?,
        ))
        .map(|decoder| decoder.has_animation())
        .map_err(|_| PdfError::new(PdfErrorKind::MalformedImage))?,
        _ => false,
    };

    if is_animated {
        Err(PdfError::new(PdfErrorKind::UnsupportedImageFormat))
    } else {
        Ok(())
    }
}

pub(crate) fn inspect_image(source_path: &Path) -> Result<ImageInfo, PdfError> {
    let decoded = decode_visually_oriented_image(source_path)?;
    Ok(ImageInfo {
        width_pixels: decoded.image.width(),
        height_pixels: decoded.image.height(),
    })
}

fn calculate_page_layout(
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

fn validate_source(source_path: &Path) -> Result<(), PdfError> {
    match fs::metadata(source_path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(PdfError::new(PdfErrorKind::SourceNotFile)),
        Err(error) => Err(map_source_io_error(error)),
    }
}

fn validate_extension(source_path: &Path) -> Result<(), PdfError> {
    if source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp"
            )
        })
    {
        Ok(())
    } else {
        Err(PdfError::new(PdfErrorKind::UnsupportedImageFormat))
    }
}

fn map_source_io_error(error: std::io::Error) -> PdfError {
    match error.kind() {
        std::io::ErrorKind::NotFound => PdfError::new(PdfErrorKind::SourceNotFound),
        _ => PdfError::new(PdfErrorKind::SourceUnreadable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
