use std::io::{Seek, Write};
use std::path::Path;

use ilikepdf_pdf::{PdfDocumentInfo, PdfDpiRenderRequest, PdfImageFormat, RenderedPage};

use crate::ApplicationResult;

pub(super) trait PdfBackend {
    fn inspect_document(&self, source_path: &Path) -> ApplicationResult<PdfDocumentInfo>;

    fn render_page_to_image_at_dpi(
        &self,
        request: PdfDpiRenderRequest,
        format: PdfImageFormat,
        output: &mut (impl Write + Seek),
    ) -> ApplicationResult<RenderedPage>;
}

pub(super) struct NativePdfBackend;

impl PdfBackend for NativePdfBackend {
    fn inspect_document(&self, source_path: &Path) -> ApplicationResult<PdfDocumentInfo> {
        ilikepdf_pdf::inspect_document(source_path).map_err(Into::into)
    }

    fn render_page_to_image_at_dpi(
        &self,
        request: PdfDpiRenderRequest,
        format: PdfImageFormat,
        output: &mut (impl Write + Seek),
    ) -> ApplicationResult<RenderedPage> {
        ilikepdf_pdf::render_page_to_image_at_dpi(request, format, output).map_err(Into::into)
    }
}

impl PdfBackend for ilikepdf_pdf::PdfRenderer {
    fn inspect_document(&self, source_path: &Path) -> ApplicationResult<PdfDocumentInfo> {
        self.inspect_document(source_path).map_err(Into::into)
    }

    fn render_page_to_image_at_dpi(
        &self,
        request: PdfDpiRenderRequest,
        format: PdfImageFormat,
        output: &mut (impl Write + Seek),
    ) -> ApplicationResult<RenderedPage> {
        self.render_page_to_image_at_dpi(request, format, output)
            .map_err(Into::into)
    }
}
