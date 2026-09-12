use std::path::Path;
use std::sync::OnceLock;

use pdfium_render::prelude::Pdfium;

use crate::{PdfError, PdfErrorKind};

static PDFIUM: OnceLock<Result<Pdfium, PdfError>> = OnceLock::new();

pub(crate) fn pdfium() -> Result<&'static Pdfium, PdfError> {
    match PDFIUM.get_or_init(load_bundled) {
        Ok(pdfium) => Ok(pdfium),
        Err(error) => Err(error.clone()),
    }
}

fn load_bundled() -> Result<Pdfium, PdfError> {
    let library = std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
        .map(|directory| directory.join(Pdfium::pdfium_platform_library_name()))
        .filter(|path| path.is_file())
        .ok_or_else(|| PdfError::new(PdfErrorKind::RuntimeUnavailable))?;
    load(&library)
}

pub(crate) fn load(library_path: &Path) -> Result<Pdfium, PdfError> {
    Pdfium::bind_to_library(library_path)
        .map(Pdfium::new)
        .map_err(|_| PdfError::new(PdfErrorKind::RuntimeUnavailable))
}

#[cfg(test)]
mod tests;
