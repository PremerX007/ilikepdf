#![forbid(unsafe_code)]
//! Application workflows and stable domain-facing contracts for iLikePDF.
//!
//! Consumers use the crate-root façade. Internal module placement is private so
//! workflows can be reorganized without creating additional public API paths.

mod application;
mod error;

pub use application::app_info::{ApplicationInfo, get_application_info};
pub use application::image_to_pdf::{
    CreateImagePdfRequest, ImagePdfFailure, ImagePdfMargin, ImagePdfOrientation, ImagePdfPageSize,
    ImagePdfProgress, ImagePdfResult, create_pdfs_from_images,
};
pub use application::pdf_to_images::{
    ExportPdfBatchRequest, ExportPdfToImagesRequest, PdfBatchDestinationMode,
    PdfBatchDocumentResult, PdfBatchFailure, PdfBatchProgress, PdfBatchResult, PdfDocumentInfo,
    PdfExportFailure, PdfExportFormat, PdfExportProgress, PdfExportQuality, PdfExportResult,
    PdfPageSize, RenderPdfPageRequest, RenderPdfPageResult, export_pdf_batch_to_images,
    export_pdf_to_images, inspect_pdf_document, render_pdf_page,
};
pub use application::structural_pdf::{
    MergePdfFailure, MergePdfProgress, MergePdfRequest, MergePdfResult, MergePdfStage,
    RewriteStructuralPdfRequest, SplitPdfFailure, SplitPdfMode, SplitPdfPageRange, SplitPdfPart,
    SplitPdfProgress, SplitPdfRequest, SplitPdfResult, SplitPdfSourceInfo, SplitPdfStage,
    StructuralPdfEngine, StructuralPdfEngineFamily, StructuralPdfEngineInfo, StructuralPdfError,
    StructuralPdfMergeRequest, StructuralPdfOperationResult, StructuralPdfPageRangeRequest,
    StructuralPdfRewriteResult, StructuralPdfValidation, StructuralPdfVersion,
    inspect_split_pdf_source, merge_pdf, probe_structural_pdf_engine, rewrite_structural_pdf,
    split_pdf, validate_structural_pdf,
};
pub use error::{ApplicationError, ApplicationErrorCode, ApplicationResult};

#[cfg(test)]
mod test_support;
