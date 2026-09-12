use std::fs;
use std::io::{Seek, Write};
use std::path::Path;

use image::{ImageFormat, codecs::jpeg::JpegEncoder};
use pdfium_render::prelude::{PdfRenderConfig, Pdfium};

use crate::{
    PdfDocumentInfo, PdfDpiRenderRequest, PdfError, PdfErrorKind, PdfImageFormat, PdfPageSize,
    PdfRenderRequest, RenderedPage,
};

const PDF_POINTS_PER_INCH: f32 = 72.0;

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
    let image = bitmap
        .as_image()
        .map_err(|_| PdfError::new(PdfErrorKind::RenderFailed))?;

    let encoded = match format {
        PdfImageFormat::Png => image.write_to(output, ImageFormat::Png),
        PdfImageFormat::Jpg => {
            JpegEncoder::new_with_quality(output, 90).encode_image(&image.to_rgb8())
        }
    };
    encoded.map_err(|error| match error {
        image::ImageError::IoError(_) => PdfError::new(PdfErrorKind::OutputWriteFailed),
        _ => PdfError::new(PdfErrorKind::EncodeFailed),
    })?;

    Ok(RenderedPage {
        width_pixels,
        height_pixels,
    })
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
