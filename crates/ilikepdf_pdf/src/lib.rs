#![forbid(unsafe_code)]
//! Native PDF infrastructure for page inspection, rendering, and image-backed PDFs.
//!
//! The free functions use the bundled application runtime. `PdfRenderer` is the
//! explicit-runtime façade used by native integration tests and alternate hosts;
//! both paths delegate to the same private PDFium implementation modules.

mod error;
mod image_pdf;
mod page_geometry;
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
pub use page_geometry::{PdfDocumentGeometry, PdfPageBox, PdfPageGeometry, PdfPageRotation};

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

#[derive(Clone, PartialEq, Eq)]
pub struct PdfPageRasterRequest {
    pub source_path: PathBuf,
    pub page_index: u32,
    pub width_pixels: u32,
    pub height_pixels: u32,
}

impl PdfPageRasterRequest {
    /// Two such decoded pages fit the editor's 256 MiB retained-image budget.
    pub const MAX_PIXELS: u64 = 32 * 1024 * 1024;
    pub const MAX_AXIS: u32 = 8192;
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
    pub fn render_page_raster(
        &self,
        request: PdfPageRasterRequest,
        output: &mut (impl Write + Seek),
    ) -> Result<RenderedPage, PdfError> {
        pdfium_engine::render_page_raster(&self.pdfium, request, output)
    }
    /// Loads a controlled runtime path for native integration tests and hosts.
    pub fn from_library_path(library_path: &Path) -> Result<Self, PdfError> {
        runtime::load(library_path).map(|pdfium| Self { pdfium })
    }

    pub fn inspect_document(&self, source_path: &Path) -> Result<PdfDocumentInfo, PdfError> {
        pdfium_engine::inspect_document(&self.pdfium, source_path)
    }

    pub fn inspect_page_geometry(
        &self,
        source_path: &Path,
    ) -> Result<PdfDocumentGeometry, PdfError> {
        pdfium_engine::inspect_page_geometry(&self.pdfium, source_path)
    }

    pub fn inspect_document_with_password(
        &self,
        source_path: &Path,
        password: &str,
    ) -> Result<PdfDocumentInfo, PdfError> {
        pdfium_engine::inspect_document_with_password(&self.pdfium, source_path, password)
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

/// Reads ordered page geometry without changing or rendering the source document.
pub fn inspect_page_geometry(source_path: &Path) -> Result<PdfDocumentGeometry, PdfError> {
    pdfium_engine::inspect_page_geometry(runtime::pdfium()?, source_path)
}

pub fn inspect_document_with_password(
    source_path: &Path,
    password: &str,
) -> Result<PdfDocumentInfo, PdfError> {
    pdfium_engine::inspect_document_with_password(runtime::pdfium()?, source_path, password)
}

pub fn render_page_to_png(
    request: PdfRenderRequest,
    output: &mut (impl Write + Seek),
) -> Result<RenderedPage, PdfError> {
    pdfium_engine::render_page_to_png(runtime::pdfium()?, request, output)
}

pub fn render_page_raster(
    request: PdfPageRasterRequest,
    output: &mut (impl Write + Seek),
) -> Result<RenderedPage, PdfError> {
    pdfium_engine::render_page_raster(runtime::pdfium()?, request, output)
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
