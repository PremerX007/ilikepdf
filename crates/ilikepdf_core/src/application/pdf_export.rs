use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};

use ilikepdf_pdf::{PdfDocumentInfo, PdfDpiRenderRequest, RenderedPage};

use super::png_output::{PendingPngOutput, validate_output_directory};
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

const STANDARD_DPI: u16 = 150;
const HIGH_QUALITY_DPI: u16 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfExportQuality {
    Standard,
    HighQuality,
}

impl PdfExportQuality {
    pub const fn dpi(self) -> u16 {
        match self {
            Self::Standard => STANDARD_DPI,
            Self::HighQuality => HIGH_QUALITY_DPI,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPdfToImagesRequest {
    pub source_path: PathBuf,
    pub destination_directory: PathBuf,
    pub quality: PdfExportQuality,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfExportProgress {
    pub total_page_count: u32,
    pub completed_page_count: u32,
    pub current_page: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfExportResult {
    pub total_page_count: u32,
    pub completed_page_count: u32,
    pub output_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfExportFailure {
    pub total_page_count: u32,
    pub completed_page_count: u32,
    pub current_page: Option<u32>,
    pub output_files: Vec<PathBuf>,
    pub error: ApplicationError,
}

pub fn export_pdf_to_images(
    request: ExportPdfToImagesRequest,
    on_progress: impl FnMut(PdfExportProgress),
) -> Result<PdfExportResult, PdfExportFailure> {
    export_pdf_to_images_with_backend(&NativePdfBackend, request, on_progress)
}

trait PdfBackend {
    fn inspect_document(&self, source_path: &Path) -> ApplicationResult<PdfDocumentInfo>;

    fn render_page_to_png_at_dpi(
        &self,
        request: PdfDpiRenderRequest,
        output: &mut (impl Write + Seek),
    ) -> ApplicationResult<RenderedPage>;
}

struct NativePdfBackend;

impl PdfBackend for NativePdfBackend {
    fn inspect_document(&self, source_path: &Path) -> ApplicationResult<PdfDocumentInfo> {
        ilikepdf_pdf::inspect_document(source_path).map_err(Into::into)
    }

    fn render_page_to_png_at_dpi(
        &self,
        request: PdfDpiRenderRequest,
        output: &mut (impl Write + Seek),
    ) -> ApplicationResult<RenderedPage> {
        ilikepdf_pdf::render_page_to_png_at_dpi(request, output).map_err(Into::into)
    }
}

impl PdfBackend for ilikepdf_pdf::PdfRenderer {
    fn inspect_document(&self, source_path: &Path) -> ApplicationResult<PdfDocumentInfo> {
        self.inspect_document(source_path).map_err(Into::into)
    }

    fn render_page_to_png_at_dpi(
        &self,
        request: PdfDpiRenderRequest,
        output: &mut (impl Write + Seek),
    ) -> ApplicationResult<RenderedPage> {
        self.render_page_to_png_at_dpi(request, output)
            .map_err(Into::into)
    }
}

fn export_pdf_to_images_with_backend(
    backend: &impl PdfBackend,
    request: ExportPdfToImagesRequest,
    mut on_progress: impl FnMut(PdfExportProgress),
) -> Result<PdfExportResult, PdfExportFailure> {
    let info = backend
        .inspect_document(&request.source_path)
        .map_err(PdfExportFailure::before_start)?;
    let total_page_count = info.page_count;
    validate_output_directory(&request.destination_directory)
        .map_err(|error| PdfExportFailure::with_total(total_page_count, error))?;

    let output_plan = output_plan(
        &request.source_path,
        &request.destination_directory,
        total_page_count,
    )
    .map_err(|error| PdfExportFailure::with_total(total_page_count, error))?;
    if output_plan
        .directory_to_create
        .as_ref()
        .is_some_and(|path| path.exists())
        || output_plan.destinations.iter().any(|path| path.exists())
    {
        return Err(PdfExportFailure::with_total(
            total_page_count,
            output_collision(),
        ));
    }
    if let Some(directory) = &output_plan.directory_to_create {
        create_output_directory(directory)
            .map_err(|error| PdfExportFailure::with_total(total_page_count, error))?;
    }

    let destinations = output_plan.destinations;

    let mut output_files = Vec::with_capacity(destinations.len());
    if total_page_count == 0 {
        return Ok(PdfExportResult {
            total_page_count,
            completed_page_count: 0,
            output_files,
        });
    }

    on_progress(PdfExportProgress {
        total_page_count,
        completed_page_count: 0,
        current_page: Some(1),
    });

    for (page_index, destination) in destinations.into_iter().enumerate() {
        let current_page = u32::try_from(page_index + 1).expect("page count is represented by u32");
        let mut output = PendingPngOutput::for_destination(&destination).map_err(|error| {
            PdfExportFailure::during_export(
                total_page_count,
                current_page,
                output_files.clone(),
                error,
            )
        })?;
        backend
            .render_page_to_png_at_dpi(
                PdfDpiRenderRequest {
                    source_path: request.source_path.clone(),
                    page_index: current_page - 1,
                    dpi: request.quality.dpi(),
                },
                output.file.as_file_mut(),
            )
            .map_err(|error| {
                PdfExportFailure::during_export(
                    total_page_count,
                    current_page,
                    output_files.clone(),
                    error,
                )
            })?;
        let published = output.publish().map_err(|error| {
            PdfExportFailure::during_export(
                total_page_count,
                current_page,
                output_files.clone(),
                error,
            )
        })?;
        output_files.push(published);

        on_progress(PdfExportProgress {
            total_page_count,
            completed_page_count: current_page,
            current_page: Some(current_page.saturating_add(1).min(total_page_count)),
        });
    }

    Ok(PdfExportResult {
        total_page_count,
        completed_page_count: total_page_count,
        output_files,
    })
}

impl PdfExportFailure {
    fn before_start(error: ApplicationError) -> Self {
        Self {
            total_page_count: 0,
            completed_page_count: 0,
            current_page: None,
            output_files: Vec::new(),
            error,
        }
    }

    fn with_total(total_page_count: u32, error: ApplicationError) -> Self {
        Self {
            total_page_count,
            completed_page_count: 0,
            current_page: None,
            output_files: Vec::new(),
            error,
        }
    }

    fn during_export(
        total_page_count: u32,
        current_page: u32,
        output_files: Vec<PathBuf>,
        error: ApplicationError,
    ) -> Self {
        Self {
            total_page_count,
            completed_page_count: u32::try_from(output_files.len())
                .expect("output count cannot exceed the PDF page count"),
            current_page: Some(current_page),
            output_files,
            error,
        }
    }
}

struct PdfExportOutputPlan {
    directory_to_create: Option<PathBuf>,
    destinations: Vec<PathBuf>,
}

fn output_plan(
    source_path: &Path,
    selected_directory: &Path,
    page_count: u32,
) -> ApplicationResult<PdfExportOutputPlan> {
    if page_count == 0 {
        return Ok(PdfExportOutputPlan {
            directory_to_create: None,
            destinations: Vec::new(),
        });
    }

    let source_stem = source_path
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| {
            ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "The source PDF must have a file name",
            )
        })?;
    let (output_directory, directory_to_create) = if page_count == 1 {
        (selected_directory.to_path_buf(), None)
    } else {
        let output_directory = selected_directory.join(source_stem);
        (output_directory.clone(), Some(output_directory))
    };
    let padding = page_count.to_string().len().max(4);
    let destinations = (1..=page_count)
        .map(|page_number| {
            output_directory.join(output_file_name(source_stem, page_number, padding))
        })
        .collect();

    Ok(PdfExportOutputPlan {
        directory_to_create,
        destinations,
    })
}

fn output_file_name(source_stem: &OsStr, page_number: u32, padding: usize) -> OsString {
    let mut name = source_stem.to_os_string();
    name.push(format!("-page-{page_number:0padding$}.png"));
    name
}

fn create_output_directory(directory: &Path) -> ApplicationResult<()> {
    fs::create_dir(directory).map_err(|error| match error.kind() {
        std::io::ErrorKind::AlreadyExists => output_collision(),
        std::io::ErrorKind::PermissionDenied => ApplicationError::new(
            ApplicationErrorCode::PermissionDenied,
            "Permission was denied while creating the output folder",
        ),
        _ => ApplicationError::new(
            ApplicationErrorCode::OutputWriteFailed,
            "The output folder could not be created",
        ),
    })
}

fn output_collision() -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::OutputAlreadyExists,
        "The required output file or folder already exists",
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ilikepdf_pdf::PdfRenderer;

    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("ilikepdf_pdf")
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn renderer() -> &'static PdfRenderer {
        crate::test_pdf_renderer()
    }

    fn working_copy(
        fixture_name: &str,
        source_name: &str,
    ) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let source = directory.path().join(source_name);
        fs::copy(fixture(fixture_name), &source).expect("fixture should be copied");
        let destination = directory.path().join("output");
        fs::create_dir(&destination).expect("output directory should be created");
        (directory, source, destination)
    }

    #[test]
    fn exports_two_pages_with_deterministic_names_and_progress() {
        let (_directory, source, destination) = working_copy("two_page.pdf", "invoice.pdf");
        let source_before = fs::read(&source).expect("fixture should be readable");
        let mut progress = Vec::new();

        let result = export_pdf_to_images_with_backend(
            renderer(),
            ExportPdfToImagesRequest {
                source_path: source.clone(),
                destination_directory: destination.clone(),
                quality: PdfExportQuality::Standard,
            },
            |update| progress.push(update),
        )
        .expect("the fixture should export");

        assert_eq!(result.total_page_count, 2);
        assert_eq!(result.completed_page_count, 2);
        assert_eq!(
            result
                .output_files
                .iter()
                .map(|path| path.file_name().expect("output has a name"))
                .collect::<Vec<_>>(),
            ["invoice-page-0001.png", "invoice-page-0002.png"]
        );
        assert_eq!(
            result.output_files[0].parent(),
            Some(destination.join("invoice").as_path())
        );
        assert_eq!(png_dimensions(&result.output_files[0]), (625, 417));
        assert_eq!(png_dimensions(&result.output_files[1]), (417, 625));
        assert_eq!(
            progress.first().expect("start progress").current_page,
            Some(1)
        );
        assert_eq!(
            progress
                .last()
                .expect("final progress")
                .completed_page_count,
            2
        );
        assert_eq!(
            fs::read(source).expect("fixture should remain readable"),
            source_before
        );
    }

    #[test]
    fn one_page_exports_beside_the_selected_directory_without_a_subdirectory() {
        let (_directory, source, destination) = working_copy("one_page.pdf", "cover.pdf");
        let source_before = fs::read(&source).expect("fixture should be readable");

        let result = export_pdf_to_images_with_backend(
            renderer(),
            ExportPdfToImagesRequest {
                source_path: source.clone(),
                destination_directory: destination.clone(),
                quality: PdfExportQuality::Standard,
            },
            |_| {},
        )
        .expect("one-page PDF should export");

        assert_eq!(
            result.output_files,
            [destination.join("cover-page-0001.png")]
        );
        assert!(!destination.join("cover").exists());
        assert_eq!(png_dimensions(&result.output_files[0]), (625, 417));
        assert_eq!(
            fs::read(source).expect("source should remain readable"),
            source_before
        );
    }

    #[test]
    fn unicode_source_stem_is_preserved_in_the_output_name() {
        let (_directory, source, destination) = working_copy("one_page.pdf", "ใบแจ้งหนี้.pdf");

        let result = export_pdf_to_images_with_backend(
            renderer(),
            ExportPdfToImagesRequest {
                source_path: source,
                destination_directory: destination.clone(),
                quality: PdfExportQuality::Standard,
            },
            |_| {},
        )
        .expect("Unicode source name should export");

        assert_eq!(
            result.output_files,
            [destination.join("ใบแจ้งหนี้-page-0001.png")]
        );
    }

    #[test]
    fn high_quality_export_is_300_dpi_and_larger_than_standard() {
        let source = fixture("two_page.pdf");
        let standard_directory = tempfile::tempdir().expect("standard directory should be created");
        let high_directory = tempfile::tempdir().expect("high directory should be created");
        let backend = renderer();

        let standard = export_pdf_to_images_with_backend(
            backend,
            ExportPdfToImagesRequest {
                source_path: source.clone(),
                destination_directory: standard_directory.path().to_path_buf(),
                quality: PdfExportQuality::Standard,
            },
            |_| {},
        )
        .expect("standard export should succeed");
        let high = export_pdf_to_images_with_backend(
            backend,
            ExportPdfToImagesRequest {
                source_path: source,
                destination_directory: high_directory.path().to_path_buf(),
                quality: PdfExportQuality::HighQuality,
            },
            |_| {},
        )
        .expect("high-quality export should succeed");

        let standard_size = png_dimensions(&standard.output_files[0]);
        let high_size = png_dimensions(&high.output_files[0]);
        assert_eq!(standard_size, (625, 417));
        assert_eq!(high_size, (1250, 833));
        assert!(high_size.0 > standard_size.0);
        assert!(high_size.1 > standard_size.1);
        assert!((high_size.0 as f64 / high_size.1 as f64 - 1.5).abs() < 0.01);
    }

    #[test]
    fn existing_multi_page_output_directory_is_a_collision() {
        let (_directory, source, destination) = working_copy("two_page.pdf", "invoice.pdf");
        let existing_directory = destination.join("invoice");
        fs::create_dir(&existing_directory).expect("collision directory should be created");
        let existing = existing_directory.join("keep.txt");
        fs::write(&existing, b"existing").expect("collision content should be created");

        let failure = export_pdf_to_images_with_backend(
            renderer(),
            ExportPdfToImagesRequest {
                source_path: source,
                destination_directory: destination,
                quality: PdfExportQuality::Standard,
            },
            |_| {},
        )
        .expect_err("a collision should fail before rendering");

        assert_eq!(
            failure.error.code,
            ApplicationErrorCode::OutputAlreadyExists
        );
        assert_eq!(
            fs::read(existing).expect("collision should remain"),
            b"existing"
        );
        assert_eq!(
            fs::read_dir(existing_directory)
                .expect("collision directory should remain readable")
                .count(),
            1
        );
    }

    #[test]
    fn existing_single_page_output_is_not_overwritten() {
        let (_directory, source, destination) = working_copy("one_page.pdf", "cover.pdf");
        let existing = destination.join("cover-page-0001.png");
        fs::write(&existing, b"existing").expect("collision should be created");

        let failure = export_pdf_to_images_with_backend(
            renderer(),
            ExportPdfToImagesRequest {
                source_path: source,
                destination_directory: destination,
                quality: PdfExportQuality::Standard,
            },
            |_| {},
        )
        .expect_err("an existing output should fail before rendering");

        assert_eq!(
            failure.error.code,
            ApplicationErrorCode::OutputAlreadyExists
        );
        assert_eq!(
            fs::read(existing).expect("collision should remain"),
            b"existing"
        );
    }

    #[test]
    fn page_failure_reports_already_published_outputs_and_removes_pending_output() {
        let destination = tempfile::tempdir().expect("temporary directory should be created");
        let output_directory = destination.path().join("two_page");

        let failure = export_pdf_to_images_with_backend(
            &FailOnSecondPageBackend {
                renderer: renderer(),
            },
            ExportPdfToImagesRequest {
                source_path: fixture("two_page.pdf"),
                destination_directory: destination.path().to_path_buf(),
                quality: PdfExportQuality::Standard,
            },
            |_| {},
        )
        .expect_err("the second page should fail");

        assert_eq!(failure.error.code, ApplicationErrorCode::RenderingFailed);
        assert_eq!(failure.completed_page_count, 1);
        assert_eq!(failure.current_page, Some(2));
        assert_eq!(failure.output_files.len(), 1);
        assert!(output_directory.join("two_page-page-0001.png").is_file());
        assert!(!output_directory.join("two_page-page-0002.png").exists());
        assert_eq!(
            fs::read_dir(output_directory)
                .expect("destination should be readable")
                .count(),
            1
        );
    }

    #[test]
    fn malformed_input_maps_to_a_structured_error() {
        let destination = tempfile::tempdir().expect("temporary directory should be created");

        let failure = export_pdf_to_images_with_backend(
            renderer(),
            ExportPdfToImagesRequest {
                source_path: fixture("malformed.pdf"),
                destination_directory: destination.path().to_path_buf(),
                quality: PdfExportQuality::Standard,
            },
            |_| {},
        )
        .expect_err("malformed input should fail");

        assert_eq!(failure.error.code, ApplicationErrorCode::InvalidPdf);
    }

    #[test]
    fn invalid_destination_maps_to_a_structured_error() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let missing = directory.path().join("missing");

        let failure = export_pdf_to_images_with_backend(
            renderer(),
            ExportPdfToImagesRequest {
                source_path: fixture("two_page.pdf"),
                destination_directory: missing,
                quality: PdfExportQuality::Standard,
            },
            |_| {},
        )
        .expect_err("an invalid destination should fail");

        assert_eq!(
            failure.error.code,
            ApplicationErrorCode::InvalidOutputDirectory
        );
    }

    #[test]
    fn filename_padding_has_a_four_digit_minimum() {
        let plan = output_plan(Path::new("invoice.pdf"), Path::new("output"), 1_000)
            .expect("output plan should be valid");

        assert_eq!(
            plan.destinations[0],
            Path::new("output")
                .join("invoice")
                .join("invoice-page-0001.png")
        );
        assert_eq!(
            plan.destinations[9],
            Path::new("output")
                .join("invoice")
                .join("invoice-page-0010.png")
        );
        assert_eq!(
            plan.destinations[999],
            Path::new("output")
                .join("invoice")
                .join("invoice-page-1000.png")
        );
    }

    fn png_dimensions(path: &Path) -> (u32, u32) {
        let bytes = fs::read(path).expect("PNG should be readable");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let width = u32::from_be_bytes(bytes[16..20].try_into().expect("PNG width bytes"));
        let height = u32::from_be_bytes(bytes[20..24].try_into().expect("PNG height bytes"));
        (width, height)
    }

    struct FailOnSecondPageBackend {
        renderer: &'static PdfRenderer,
    }

    impl PdfBackend for FailOnSecondPageBackend {
        fn inspect_document(&self, source_path: &Path) -> ApplicationResult<PdfDocumentInfo> {
            self.renderer
                .inspect_document(source_path)
                .map_err(Into::into)
        }

        fn render_page_to_png_at_dpi(
            &self,
            request: PdfDpiRenderRequest,
            output: &mut (impl Write + Seek),
        ) -> ApplicationResult<RenderedPage> {
            if request.page_index == 1 {
                return Err(ApplicationError::new(
                    ApplicationErrorCode::RenderingFailed,
                    "The PDF page could not be rendered",
                ));
            }
            self.renderer
                .render_page_to_png_at_dpi(request, output)
                .map_err(Into::into)
        }
    }
}
