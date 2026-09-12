use std::io::{BufWriter, Seek, Write};

use image::{
    ExtendedColorType, ImageEncoder,
    codecs::png::{CompressionType, FilterType, PngEncoder},
};
use jpeg_rusturbo::{ChromaSubsampling, JpegEncoder};
use pdfium_render::prelude::PdfBitmapFormat;

use crate::{PdfError, PdfErrorKind, PdfImageFormat};

const OUTPUT_BUFFER_CAPACITY: usize = 64 * 1024;
const JPEG_QUALITY: u8 = 90;

pub(super) fn encode_bitmap_pixels(
    pixels: &[u8],
    bitmap_format: PdfBitmapFormat,
    width_pixels: u32,
    height_pixels: u32,
    format: PdfImageFormat,
    output: &mut (impl Write + Seek),
) -> Result<(), PdfError> {
    let mut buffered_output = BufWriter::with_capacity(OUTPUT_BUFFER_CAPACITY, output);

    match format {
        PdfImageFormat::Png => PngEncoder::new_with_quality(
            &mut buffered_output,
            CompressionType::Fast,
            FilterType::Adaptive,
        )
        .write_image(
            pixels,
            width_pixels,
            height_pixels,
            encoded_color_type(bitmap_format),
        )
        .map_err(map_image_encoding_error),
        PdfImageFormat::Jpg => {
            let mut encoder = JpegEncoder::new_with_quality(&mut buffered_output, JPEG_QUALITY);
            encoder.set_subsampling(ChromaSubsampling::Yuv444);
            encoder.set_threads(0);
            match bitmap_format {
                PdfBitmapFormat::Gray => {
                    encoder.encode_grayscale(pixels, width_pixels, height_pixels)
                }
                PdfBitmapFormat::BGR | PdfBitmapFormat::BGRx | PdfBitmapFormat::BGRA => {
                    encoder.encode_rgba(pixels, width_pixels, height_pixels)
                }
            }
            .map_err(map_jpeg_encoding_error)
        }
    }?;

    buffered_output
        .flush()
        .map_err(|_| PdfError::new(PdfErrorKind::OutputWriteFailed))
}

fn encoded_color_type(bitmap_format: PdfBitmapFormat) -> ExtendedColorType {
    match bitmap_format {
        PdfBitmapFormat::Gray => ExtendedColorType::L8,
        PdfBitmapFormat::BGR | PdfBitmapFormat::BGRx | PdfBitmapFormat::BGRA => {
            ExtendedColorType::Rgba8
        }
    }
}

fn map_image_encoding_error(error: image::ImageError) -> PdfError {
    match error {
        image::ImageError::IoError(_) => PdfError::new(PdfErrorKind::OutputWriteFailed),
        _ => PdfError::new(PdfErrorKind::EncodeFailed),
    }
}

fn map_jpeg_encoding_error(error: std::io::Error) -> PdfError {
    match error.kind() {
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::Unsupported => {
            PdfError::new(PdfErrorKind::EncodeFailed)
        }
        _ => PdfError::new(PdfErrorKind::OutputWriteFailed),
    }
}
