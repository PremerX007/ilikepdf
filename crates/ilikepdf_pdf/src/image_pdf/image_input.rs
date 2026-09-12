use std::fs;
use std::io::BufReader;
use std::path::Path;

use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use image::{codecs::png::PngDecoder, codecs::webp::WebPDecoder};

use super::ImageInfo;
use crate::{PdfError, PdfErrorKind};

pub(super) struct DecodedImage {
    pub(super) image: DynamicImage,
    pub(super) can_embed_source_jpeg: bool,
}

pub(super) fn decode_visually_oriented_image(source_path: &Path) -> Result<DecodedImage, PdfError> {
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
