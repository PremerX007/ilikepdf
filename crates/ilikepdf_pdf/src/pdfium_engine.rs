use std::fs;
use std::io::{BufWriter, Seek, Write};
use std::path::Path;

use image::{
    ExtendedColorType, ImageEncoder,
    codecs::png::{CompressionType, FilterType, PngEncoder},
};
use jpeg_rusturbo::{ChromaSubsampling, JpegEncoder};
use pdfium_render::prelude::{PdfBitmapFormat, PdfRenderConfig, Pdfium};

use crate::{
    PdfDocumentInfo, PdfDpiRenderRequest, PdfError, PdfErrorKind, PdfImageFormat, PdfPageSize,
    PdfRenderRequest, RenderedPage,
};

const PDF_POINTS_PER_INCH: f32 = 72.0;
const IMAGE_OUTPUT_BUFFER_CAPACITY: usize = 64 * 1024;
const JPEG_QUALITY: u8 = 90;

pub(crate) fn inspect_document(
    pdfium: &Pdfium,
    source_path: &Path,
) -> Result<PdfDocumentInfo, PdfError> {
    validate_source(source_path)?;
    inspect_with_pdfium(pdfium, source_path)
}

pub(crate) fn render_page_to_png(
    pdfium: &Pdfium,
    request: PdfRenderRequest,
    output: &mut (impl Write + Seek),
) -> Result<RenderedPage, PdfError> {
    validate_source(&request.source_path)?;
    let document = pdfium
        .load_pdf_from_file(&request.source_path, None)
        .map_err(|_| PdfError::new(PdfErrorKind::InvalidDocument))?;
    let page_index = i32::try_from(request.page_index)
        .map_err(|_| PdfError::new(PdfErrorKind::PageOutOfBounds))?;
    if page_index >= document.pages().len() {
        return Err(PdfError::new(PdfErrorKind::PageOutOfBounds));
    }
    let target_width = i32::try_from(request.target_width)
        .map_err(|_| PdfError::new(PdfErrorKind::RenderFailed))?;
    let page = document
        .pages()
        .get(page_index)
        .map_err(|_| PdfError::new(PdfErrorKind::PageOutOfBounds))?;
    render_page_with_config(
        &page,
        PdfRenderConfig::new().set_target_width(target_width),
        PdfImageFormat::Png,
        output,
    )
}

pub(crate) fn render_page_to_png_at_dpi(
    pdfium: &Pdfium,
    request: PdfDpiRenderRequest,
    output: &mut (impl Write + Seek),
) -> Result<RenderedPage, PdfError> {
    render_page_to_image_at_dpi(pdfium, request, PdfImageFormat::Png, output)
}

pub(crate) fn render_page_to_image_at_dpi(
    pdfium: &Pdfium,
    request: PdfDpiRenderRequest,
    format: PdfImageFormat,
    output: &mut (impl Write + Seek),
) -> Result<RenderedPage, PdfError> {
    validate_source(&request.source_path)?;
    if request.dpi == 0 {
        return Err(PdfError::new(PdfErrorKind::RenderFailed));
    }

    let document = pdfium
        .load_pdf_from_file(&request.source_path, None)
        .map_err(|_| PdfError::new(PdfErrorKind::InvalidDocument))?;
    let page_index = i32::try_from(request.page_index)
        .map_err(|_| PdfError::new(PdfErrorKind::PageOutOfBounds))?;
    if page_index >= document.pages().len() {
        return Err(PdfError::new(PdfErrorKind::PageOutOfBounds));
    }
    let page = document
        .pages()
        .get(page_index)
        .map_err(|_| PdfError::new(PdfErrorKind::PageOutOfBounds))?;
    let scale = f32::from(request.dpi) / PDF_POINTS_PER_INCH;

    render_page_with_config(
        &page,
        PdfRenderConfig::new().scale_page_by_factor(scale),
        format,
        output,
    )
}

fn render_page_with_config(
    page: &pdfium_render::prelude::PdfPage<'_>,
    config: PdfRenderConfig,
    format: PdfImageFormat,
    output: &mut (impl Write + Seek),
) -> Result<RenderedPage, PdfError> {
    let bitmap = page
        .render_with_config(&config)
        .map_err(|_| PdfError::new(PdfErrorKind::RenderFailed))?;
    let width_pixels =
        u32::try_from(bitmap.width()).map_err(|_| PdfError::new(PdfErrorKind::RenderFailed))?;
    let height_pixels =
        u32::try_from(bitmap.height()).map_err(|_| PdfError::new(PdfErrorKind::RenderFailed))?;
    let bitmap_format = bitmap
        .format()
        .map_err(|_| PdfError::new(PdfErrorKind::RenderFailed))?;
    let pixels = bitmap.as_rgba_bytes();
    let mut buffered_output = BufWriter::with_capacity(IMAGE_OUTPUT_BUFFER_CAPACITY, output);

    let encoded = match format {
        PdfImageFormat::Png => PngEncoder::new_with_quality(
            &mut buffered_output,
            CompressionType::Fast,
            FilterType::Adaptive,
        )
        .write_image(
            &pixels,
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
                    encoder.encode_grayscale(&pixels, width_pixels, height_pixels)
                }
                PdfBitmapFormat::BGR | PdfBitmapFormat::BGRx | PdfBitmapFormat::BGRA => {
                    encoder.encode_rgba(&pixels, width_pixels, height_pixels)
                }
            }
            .map_err(map_jpeg_encoding_error)
        }
    };
    encoded?;
    buffered_output
        .flush()
        .map_err(|_| PdfError::new(PdfErrorKind::OutputWriteFailed))?;

    Ok(RenderedPage {
        width_pixels,
        height_pixels,
    })
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

fn inspect_with_pdfium(pdfium: &Pdfium, source_path: &Path) -> Result<PdfDocumentInfo, PdfError> {
    let document = pdfium
        .load_pdf_from_file(source_path, None)
        .map_err(|_| PdfError::new(PdfErrorKind::InvalidDocument))?;
    let page_count = u32::try_from(document.pages().len())
        .map_err(|_| PdfError::new(PdfErrorKind::InvalidDocument))?;
    let first_page_size = if page_count == 0 {
        None
    } else {
        let page = document
            .pages()
            .get(0)
            .map_err(|_| PdfError::new(PdfErrorKind::InvalidDocument))?;
        Some(PdfPageSize {
            width_points: page.width().value,
            height_points: page.height().value,
        })
    };

    Ok(PdfDocumentInfo {
        page_count,
        first_page_size,
    })
}

fn validate_source(source_path: &Path) -> Result<(), PdfError> {
    match fs::metadata(source_path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err(PdfError::new(PdfErrorKind::SourceNotFile)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(PdfError::new(PdfErrorKind::SourceNotFound));
        }
        Err(_) => return Err(PdfError::new(PdfErrorKind::SourceUnreadable)),
    }

    fs::File::open(source_path)
        .map(|_| ())
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => PdfError::new(PdfErrorKind::SourceNotFound),
            _ => PdfError::new(PdfErrorKind::SourceUnreadable),
        })
}
