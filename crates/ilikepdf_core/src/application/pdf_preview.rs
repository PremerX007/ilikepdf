use std::path::{Path, PathBuf};

use ilikepdf_pdf::PdfRenderRequest;

use super::png_output::PendingPngOutput;
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

const MIN_RENDER_WIDTH: u32 = 64;
const MAX_RENDER_WIDTH: u32 = 8192;

#[derive(Debug, Clone, PartialEq)]
pub struct PdfPageSize {
    pub width_points: f64,
    pub height_points: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfDocumentInfo {
    pub page_count: u32,
    pub first_page_size: Option<PdfPageSize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderPdfPageRequest {
    pub source_path: PathBuf,
    pub page_index: u32,
    pub target_width: u32,
    pub destination_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderPdfPageResult {
    pub output_path: PathBuf,
    pub width_pixels: u32,
    pub height_pixels: u32,
}

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

    let mut output = PendingPngOutput::create(request.destination_path.as_deref())?;
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
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_render_widths_before_creating_output() {
        let error = render_pdf_page(RenderPdfPageRequest {
            source_path: PathBuf::from("not-opened.pdf"),
            page_index: 0,
            target_width: 63,
            destination_path: None,
        })
        .expect_err("small render widths should be rejected");

        assert_eq!(error.code, ApplicationErrorCode::InvalidRequest);
    }
}
