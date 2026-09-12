//! PDF-to-image preview, single-document export, and batch export workflows.
//!
//! This feature owns its public request/result models and shares one rendering
//! backend contract across single and batch export. Native PDF details remain in
//! `ilikepdf_pdf`; output publication remains an application-level policy.

mod backend;
mod batch;
mod errors;
mod export;
mod model;
mod plan;
mod preview;

pub use batch::export_pdf_batch_to_images;
pub use export::export_pdf_to_images;
pub use model::{
    ExportPdfBatchRequest, ExportPdfToImagesRequest, PdfBatchDestinationMode,
    PdfBatchDocumentResult, PdfBatchFailure, PdfBatchProgress, PdfBatchResult, PdfDocumentInfo,
    PdfExportFailure, PdfExportFormat, PdfExportProgress, PdfExportQuality, PdfExportResult,
    PdfPageSize, RenderPdfPageRequest, RenderPdfPageResult,
};
pub use preview::{inspect_pdf_document, render_pdf_page};
