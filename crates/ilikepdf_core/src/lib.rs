#![forbid(unsafe_code)]

pub mod application;
pub mod error;

pub use application::app_info::{ApplicationInfo, get_application_info};
pub use application::pdf_export::{
    ExportPdfToImagesRequest, PdfExportFailure, PdfExportProgress, PdfExportQuality,
    PdfExportResult, export_pdf_to_images,
};
pub use application::pdf_preview::{
    PdfDocumentInfo, PdfPageSize, RenderPdfPageRequest, RenderPdfPageResult, inspect_pdf_document,
    render_pdf_page,
};
pub use error::{ApplicationError, ApplicationErrorCode, ApplicationResult};
