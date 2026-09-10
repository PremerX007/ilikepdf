use std::path::PathBuf;

use super::application::ApplicationError;

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
    pub source_path: String,
    pub page_index: u32,
    pub target_width: u32,
    pub destination_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderPdfPageResult {
    pub output_path: String,
    pub width_pixels: u32,
    pub height_pixels: u32,
}

/// Inspects a local PDF. The path remains on this machine and is never logged.
pub fn open_pdf_document(source_path: String) -> Result<PdfDocumentInfo, ApplicationError> {
    crate::logging::record(crate::logging::Event::PdfDocumentOpenRequested);

    ilikepdf_core::inspect_pdf_document(PathBuf::from(source_path).as_path())
        .map(|info| PdfDocumentInfo {
            page_count: info.page_count,
            first_page_size: info.first_page_size.map(|size| PdfPageSize {
                width_points: size.width_points,
                height_points: size.height_points,
            }),
        })
        .map_err(Into::into)
}

/// Renders one zero-based page through the Rust PDF infrastructure.
pub fn render_pdf_page(
    request: RenderPdfPageRequest,
) -> Result<RenderPdfPageResult, ApplicationError> {
    crate::logging::record(crate::logging::Event::PdfPageRenderRequested);

    ilikepdf_core::render_pdf_page(ilikepdf_core::RenderPdfPageRequest {
        source_path: PathBuf::from(request.source_path),
        page_index: request.page_index,
        target_width: request.target_width,
        destination_path: request.destination_path.map(PathBuf::from),
    })
    .map(|result| RenderPdfPageResult {
        output_path: result.output_path.to_string_lossy().into_owned(),
        width_pixels: result.width_pixels,
        height_pixels: result.height_pixels,
    })
    .map_err(Into::into)
}
