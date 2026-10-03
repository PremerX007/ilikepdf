#![forbid(unsafe_code)]
//! Application workflows and stable domain-facing contracts for iLikePDF.
//!
//! Consumers use the crate-root façade. Internal module placement is private so
//! workflows can be reorganized without creating additional public API paths.

mod application;
mod error;
mod secret;

pub use application::app_info::{ApplicationInfo, get_application_info};
pub use application::editor::{
    EditorDocumentLayout, EditorHit, EditorPageLayout, EditorPageMetadata, EditorPageRaster,
    EditorRenderSize, EditorSession, EditorSessionId, EditorSessionState, EditorZoom,
    EditorZoomMode, PageBox, PageGeometry, PageGeometryError, PageRotation, PageTransform,
    PdfPoint, ViewportPoint, ViewportRect, open_editor_session, render_editor_page,
};
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
    OrganizePdfFailure, OrganizePdfPageItem, OrganizePdfPageRotation, OrganizePdfProgress,
    OrganizePdfRequest, OrganizePdfResult, OrganizePdfSession, OrganizePdfSource,
    OrganizePdfSourceInfo, OrganizePdfStage, PageRotationDirection, PdfEncryptionState,
    ProtectPdfFailure, ProtectPdfProgress, ProtectPdfRequest, ProtectPdfResult,
    ProtectPdfSourceInfo, ProtectPdfStage, RewriteStructuralPdfRequest, SplitPdfFailure,
    SplitPdfMode, SplitPdfPageRange, SplitPdfPart, SplitPdfProgress, SplitPdfRequest,
    SplitPdfResult, SplitPdfSourceInfo, SplitPdfStage, StructuralPdfEngine,
    StructuralPdfEngineFamily, StructuralPdfEngineInfo, StructuralPdfError,
    StructuralPdfMergeRequest, StructuralPdfOperationResult, StructuralPdfPagePlanItem,
    StructuralPdfPagePlanRequest, StructuralPdfPageRangeRequest, StructuralPdfPageRotation,
    StructuralPdfProtectRequest, StructuralPdfRewriteResult, StructuralPdfUnlockRequest,
    StructuralPdfValidation, StructuralPdfVersion, UnlockPdfFailure, UnlockPdfProgress,
    UnlockPdfRequest, UnlockPdfResult, UnlockPdfSourceInfo, UnlockPdfStage,
    default_protect_pdf_output_name, default_unlock_pdf_output_name, inspect_organize_pdf_sources,
    inspect_protect_pdf_source, inspect_split_pdf_source, inspect_unlock_pdf_source, merge_pdf,
    normalize_organize_pdf_output_name, normalize_secure_pdf_output_name, organize_pdf,
    plan_split_pdf_ranges, probe_structural_pdf_engine, protect_pdf, rewrite_structural_pdf,
    split_pdf, unlock_pdf, validate_structural_pdf,
};
pub use error::{ApplicationError, ApplicationErrorCode, ApplicationResult};
pub use secret::SecretString;

#[cfg(test)]
mod test_support;
