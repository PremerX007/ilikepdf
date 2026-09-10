#![forbid(unsafe_code)]

mod error;
mod pdfium_engine;
mod runtime;

use std::io::{Seek, Write};
use std::path::{Path, PathBuf};

use pdfium_render::prelude::Pdfium;

pub use error::{PdfError, PdfErrorKind};

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
