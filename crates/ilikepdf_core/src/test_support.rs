use std::path::Path;
use std::sync::OnceLock;

pub(crate) fn pdf_renderer() -> &'static ilikepdf_pdf::PdfRenderer {
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
