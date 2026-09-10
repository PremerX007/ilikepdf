use std::fs;
use std::io::{Seek, Write};
use std::path::Path;

use image::ImageFormat;
use pdfium_render::prelude::{PdfRenderConfig, Pdfium};

use crate::{PdfDocumentInfo, PdfError, PdfErrorKind, PdfPageSize, PdfRenderRequest, RenderedPage};

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
    let bitmap = page
        .render_with_config(&PdfRenderConfig::new().set_target_width(target_width))
        .map_err(|_| PdfError::new(PdfErrorKind::RenderFailed))?;
    let width_pixels =
        u32::try_from(bitmap.width()).map_err(|_| PdfError::new(PdfErrorKind::RenderFailed))?;
    let height_pixels =
        u32::try_from(bitmap.height()).map_err(|_| PdfError::new(PdfErrorKind::RenderFailed))?;
    let image = bitmap
        .as_image()
        .map_err(|_| PdfError::new(PdfErrorKind::RenderFailed))?;

    image
        .write_to(output, ImageFormat::Png)
        .map_err(|error| match error {
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
