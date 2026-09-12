use std::path::Path;

use ilikepdf_pdf::PdfRenderRequest;

use crate::application::output::PendingImageOutput;
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

const MIN_RENDER_WIDTH: u32 = 64;
const MAX_RENDER_WIDTH: u32 = 8192;

use super::model::{PdfDocumentInfo, PdfPageSize, RenderPdfPageRequest, RenderPdfPageResult};

pub fn inspect_pdf_document(source_path: &Path) -> ApplicationResult<PdfDocumentInfo> {
    let info = ilikepdf_pdf::inspect_document(source_path).map_err(ApplicationError::from)?;

    Ok(PdfDocumentInfo {
        page_count: info.page_count,
        first_page_size: info.first_page_size.map(|size| PdfPageSize {
            width_points: f64::from(size.width_points),
            height_points: f64::from(size.height_points),
        }),
    })
}

pub fn render_pdf_page(request: RenderPdfPageRequest) -> ApplicationResult<RenderPdfPageResult> {
    if !(MIN_RENDER_WIDTH..=MAX_RENDER_WIDTH).contains(&request.target_width) {
        return Err(ApplicationError::new(
            ApplicationErrorCode::InvalidRequest,
            "Target width must be between 64 and 8192 pixels",
        ));
    }

    let mut output = PendingImageOutput::create_png(request.destination_path.as_deref())?;
    let rendered = ilikepdf_pdf::render_page_to_png(
        PdfRenderRequest {
            source_path: request.source_path,
            page_index: request.page_index,
            target_width: request.target_width,
        },
        output.file.as_file_mut(),
    )
    .map_err(ApplicationError::from)?;
    let output_path = output.publish()?;

    Ok(RenderPdfPageResult {
        output_path,
        width_pixels: rendered.width_pixels,
        height_pixels: rendered.height_pixels,
    })
}

#[cfg(test)]
mod tests;
