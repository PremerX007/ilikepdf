use std::fs;
use std::path::{Path, PathBuf};

use ilikepdf_pdf::{PdfError, PdfErrorKind, PdfRenderRequest};
use tempfile::{Builder, NamedTempFile};

use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

const MIN_RENDER_WIDTH: u32 = 64;
const MAX_RENDER_WIDTH: u32 = 8192;

#[derive(Debug, Clone, PartialEq)]
pub struct PdfPageSize {
    pub width_points: f64,
    pub height_points: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfDocumentInfo {
    pub page_count: u32,
    pub first_page_size: Option<PdfPageSize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderPdfPageRequest {
    pub source_path: PathBuf,
    pub page_index: u32,
    pub target_width: u32,
    pub destination_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderPdfPageResult {
    pub output_path: PathBuf,
    pub width_pixels: u32,
    pub height_pixels: u32,
}

pub fn inspect_pdf_document(source_path: &Path) -> ApplicationResult<PdfDocumentInfo> {
    let info = ilikepdf_pdf::inspect_document(source_path).map_err(map_pdf_error)?;

    Ok(PdfDocumentInfo {
        page_count: info.page_count,
        first_page_size: info.first_page_size.map(|size| PdfPageSize {
            width_points: f64::from(size.width_points),
            height_points: f64::from(size.height_points),
        }),
    })
}

pub fn render_pdf_page(request: RenderPdfPageRequest) -> ApplicationResult<RenderPdfPageResult> {
    if !(MIN_RENDER_WIDTH..=MAX_RENDER_WIDTH).contains(&request.target_width) {
        return Err(ApplicationError::new(
            ApplicationErrorCode::InvalidRequest,
            "Target width must be between 64 and 8192 pixels",
        ));
    }

    let mut output = PendingPngOutput::create(request.destination_path.as_deref())?;
    let rendered = ilikepdf_pdf::render_page_to_png(
        PdfRenderRequest {
            source_path: request.source_path,
            page_index: request.page_index,
            target_width: request.target_width,
        },
        output.file.as_file_mut(),
    )
    .map_err(map_pdf_error)?;
    let output_path = output.publish()?;

    Ok(RenderPdfPageResult {
        output_path,
        width_pixels: rendered.width_pixels,
        height_pixels: rendered.height_pixels,
    })
}

struct PendingPngOutput {
    file: NamedTempFile,
    destination: Option<PathBuf>,
}

impl PendingPngOutput {
    fn create(destination: Option<&Path>) -> ApplicationResult<Self> {
        match destination {
            Some(path) => Self::for_destination(path),
            None => Self::for_preview(),
        }
    }

    fn for_destination(path: &Path) -> ApplicationResult<Self> {
        if !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
        {
            return Err(ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "The output destination must use a .png extension",
            ));
        }
        if path.exists() {
            return Err(ApplicationError::new(
                ApplicationErrorCode::OutputAlreadyExists,
                "The output destination already exists",
            ));
        }

        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty());
        let parent = match parent {
            Some(parent) => parent.to_path_buf(),
            None => std::env::current_dir().map_err(|_| output_not_writable())?,
        };
        if !parent.is_dir() {
            return Err(output_not_writable());
        }

        let file = Builder::new()
            .prefix(".ilikepdf-render-")
            .suffix(".tmp")
            .tempfile_in(parent)
            .map_err(|_| output_not_writable())?;

        Ok(Self {
            file,
            destination: Some(path.to_path_buf()),
        })
    }

    fn for_preview() -> ApplicationResult<Self> {
        let directory = std::env::temp_dir().join("ilikepdf").join("previews");
        fs::create_dir_all(&directory).map_err(|_| output_not_writable())?;
        let file = Builder::new()
            .prefix("preview-")
            .suffix(".png")
            .tempfile_in(directory)
            .map_err(|_| output_not_writable())?;

        Ok(Self {
            file,
            destination: None,
        })
    }

    fn publish(self) -> ApplicationResult<PathBuf> {
        self.file
            .as_file()
            .sync_all()
            .map_err(|_| output_not_writable())?;

        match self.destination {
            Some(destination) => self
                .file
                .persist_noclobber(&destination)
                .map(|_| destination)
                .map_err(|error| {
                    if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                        ApplicationError::new(
                            ApplicationErrorCode::OutputAlreadyExists,
                            "The output destination already exists",
                        )
                    } else {
                        output_not_writable()
                    }
                }),
            None => self
                .file
                .keep()
                .map(|(_, path)| path)
                .map_err(|_| output_not_writable()),
        }
    }
}

fn output_not_writable() -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::OutputNotWritable,
        "The output file could not be written",
    )
}

fn map_pdf_error(error: PdfError) -> ApplicationError {
    let (code, message) = match error.kind {
        PdfErrorKind::SourceNotFound => (
            ApplicationErrorCode::SourceNotFound,
            "The selected PDF no longer exists",
        ),
        PdfErrorKind::SourceNotFile => (
            ApplicationErrorCode::SourceNotFile,
            "The selected path is not a file",
        ),
        PdfErrorKind::SourceUnreadable => (
            ApplicationErrorCode::SourceUnreadable,
            "The selected PDF could not be read",
        ),
        PdfErrorKind::InvalidDocument => (
            ApplicationErrorCode::InvalidPdf,
            "The selected file is not a valid readable PDF",
        ),
        PdfErrorKind::PageOutOfBounds => (
            ApplicationErrorCode::PageOutOfBounds,
            "The requested page does not exist",
        ),
        PdfErrorKind::RuntimeUnavailable => (
            ApplicationErrorCode::PdfRuntimeUnavailable,
            "The local PDF rendering runtime is unavailable",
        ),
        PdfErrorKind::RenderFailed => (
            ApplicationErrorCode::RenderingFailed,
            "The PDF page could not be rendered",
        ),
        PdfErrorKind::EncodeFailed => (
            ApplicationErrorCode::EncodingFailed,
            "The rendered page could not be encoded as PNG",
        ),
        PdfErrorKind::OutputWriteFailed => (
            ApplicationErrorCode::OutputNotWritable,
            "The output file could not be written",
        ),
        _ => (
            ApplicationErrorCode::Internal,
            "Unable to complete the PDF operation",
        ),
    };

    ApplicationError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_render_widths_before_creating_output() {
        let error = render_pdf_page(RenderPdfPageRequest {
            source_path: PathBuf::from("not-opened.pdf"),
            page_index: 0,
            target_width: 63,
            destination_path: None,
        })
        .expect_err("small render widths should be rejected");

        assert_eq!(error.code, ApplicationErrorCode::InvalidRequest);
    }

    #[test]
    fn rejects_non_png_destinations() {
        let error = PendingPngOutput::create(Some(Path::new("preview.jpg")))
            .err()
            .expect("non-PNG destinations should be rejected");

        assert_eq!(error.code, ApplicationErrorCode::InvalidRequest);
    }

    #[test]
    fn refuses_to_replace_an_existing_output() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let destination = directory.path().join("existing.png");
        fs::write(&destination, b"existing content").expect("test output should be created");

        let error = PendingPngOutput::create(Some(&destination))
            .err()
            .expect("existing output should be rejected");

        assert_eq!(error.code, ApplicationErrorCode::OutputAlreadyExists);
        assert_eq!(
            fs::read(destination).expect("existing output should remain readable"),
            b"existing content"
        );
    }
}
