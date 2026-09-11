#![forbid(unsafe_code)]

pub mod application;
pub mod error;

pub use application::app_info::{ApplicationInfo, get_application_info};
pub use application::image_to_pdf::{
    CreateImagePdfRequest, ImagePdfFailure, ImagePdfMargin, ImagePdfOrientation, ImagePdfPageSize,
    ImagePdfProgress, ImagePdfResult, create_pdfs_from_images,
};
pub use application::pdf_export::{
    ExportPdfToImagesRequest, PdfExportFailure, PdfExportProgress, PdfExportQuality,
    PdfExportResult, export_pdf_to_images,
};
pub use application::pdf_preview::{
    PdfDocumentInfo, PdfPageSize, RenderPdfPageRequest, RenderPdfPageResult, inspect_pdf_document,
    render_pdf_page,
};
pub use error::{ApplicationError, ApplicationErrorCode, ApplicationResult};

#[cfg(test)]
pub(crate) fn test_pdf_renderer() -> &'static ilikepdf_pdf::PdfRenderer {
    use std::path::Path;
    use std::sync::OnceLock;

    static RENDERER: OnceLock<ilikepdf_pdf::PdfRenderer> = OnceLock::new();
    RENDERER.get_or_init(|| {
        let library = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("third_party")
            .join("pdfium")
            .join("windows")
            .join("x64")
            .join("pdfium.dll");
        ilikepdf_pdf::PdfRenderer::from_library_path(&library)
            .expect("vendored PDFium runtime should load")
    })
}
