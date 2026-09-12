#![forbid(unsafe_code)]

mod error;
mod image_pdf;
mod pdfium_engine;
mod runtime;

use std::io::{Seek, Write};
use std::path::{Path, PathBuf};

use pdfium_render::prelude::Pdfium;

pub use error::{PdfError, PdfErrorKind};
pub use image_pdf::{
    ImageInfo, ImagePdfLayout, ImagePdfMargin, ImagePdfOrientation, ImagePdfPageInfo,
    ImagePdfPageSize, ImagePdfRequest, ImagePdfResult,
};

#[derive(Debug, Clone, PartialEq)]
pub struct PdfPageSize {
    pub width_points: f32,
    pub height_points: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfDocumentInfo {
    pub page_count: u32,
    pub first_page_size: Option<PdfPageSize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfRenderRequest {
    pub source_path: PathBuf,
    pub page_index: u32,
    pub target_width: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfDpiRenderRequest {
    pub source_path: PathBuf,
    pub page_index: u32,
    pub dpi: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfImageFormat {
    Png,
    Jpg,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPage {
    pub width_pixels: u32,
    pub height_pixels: u32,
}

/// Engine-neutral facade over the native rendering implementation.
pub struct PdfRenderer {
    pdfium: Pdfium,
}

impl PdfRenderer {
    /// Loads a controlled runtime path for native integration tests and hosts.
    pub fn from_library_path(library_path: &Path) -> Result<Self, PdfError> {
        runtime::load(library_path).map(|pdfium| Self { pdfium })
    }

    pub fn inspect_document(&self, source_path: &Path) -> Result<PdfDocumentInfo, PdfError> {
        pdfium_engine::inspect_document(&self.pdfium, source_path)
    }

    pub fn render_page_to_png(
        &self,
        request: PdfRenderRequest,
        output: &mut (impl Write + Seek),
    ) -> Result<RenderedPage, PdfError> {
        pdfium_engine::render_page_to_png(&self.pdfium, request, output)
    }

    pub fn render_page_to_png_at_dpi(
        &self,
        request: PdfDpiRenderRequest,
        output: &mut (impl Write + Seek),
    ) -> Result<RenderedPage, PdfError> {
        pdfium_engine::render_page_to_png_at_dpi(&self.pdfium, request, output)
    }

    pub fn render_page_to_image_at_dpi(
        &self,
        request: PdfDpiRenderRequest,
        format: PdfImageFormat,
        output: &mut (impl Write + Seek),
    ) -> Result<RenderedPage, PdfError> {
        pdfium_engine::render_page_to_image_at_dpi(&self.pdfium, request, format, output)
    }

    pub fn create_image_pdf<W: Write + 'static>(
        &self,
        request: ImagePdfRequest,
        output: &mut W,
        on_page_complete: impl FnMut(ImagePdfPageInfo),
    ) -> Result<ImagePdfResult, PdfError> {
        image_pdf::create_image_pdf(&self.pdfium, request, output, on_page_complete)
    }

    pub fn inspect_image(&self, source_path: &Path) -> Result<ImageInfo, PdfError> {
        image_pdf::inspect_image(source_path)
    }
}

pub fn inspect_document(source_path: &Path) -> Result<PdfDocumentInfo, PdfError> {
    pdfium_engine::inspect_document(runtime::pdfium()?, source_path)
}

pub fn render_page_to_png(
    request: PdfRenderRequest,
    output: &mut (impl Write + Seek),
) -> Result<RenderedPage, PdfError> {
    pdfium_engine::render_page_to_png(runtime::pdfium()?, request, output)
}

pub fn render_page_to_png_at_dpi(
    request: PdfDpiRenderRequest,
    output: &mut (impl Write + Seek),
) -> Result<RenderedPage, PdfError> {
    pdfium_engine::render_page_to_png_at_dpi(runtime::pdfium()?, request, output)
}

pub fn render_page_to_image_at_dpi(
    request: PdfDpiRenderRequest,
    format: PdfImageFormat,
    output: &mut (impl Write + Seek),
) -> Result<RenderedPage, PdfError> {
    pdfium_engine::render_page_to_image_at_dpi(runtime::pdfium()?, request, format, output)
}

pub fn create_image_pdf<W: Write + 'static>(
    request: ImagePdfRequest,
    output: &mut W,
    on_page_complete: impl FnMut(ImagePdfPageInfo),
) -> Result<ImagePdfResult, PdfError> {
    image_pdf::create_image_pdf(runtime::pdfium()?, request, output, on_page_complete)
}

pub fn inspect_image(source_path: &Path) -> Result<ImageInfo, PdfError> {
    image_pdf::inspect_image(source_path)
}
