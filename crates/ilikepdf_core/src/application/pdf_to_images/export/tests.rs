use std::ffi::OsStr;
use std::fs;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;

use ilikepdf_pdf::{PdfDocumentInfo, PdfImageFormat, PdfRenderer, RenderedPage};

use super::super::model::{PdfExportFormat, PdfExportQuality};
use super::super::plan::create_numbered_output_directory;
use super::*;
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("ilikepdf_pdf")
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn renderer() -> &'static PdfRenderer {
    crate::test_support::pdf_renderer()
}

fn working_copy(fixture_name: &str, source_name: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
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
            format: PdfExportFormat::Png,
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
            format: PdfExportFormat::Png,
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
fn one_page_jpg_export_uses_jpg_naming_and_never_clobbers() {
    let (_directory, source, destination) = working_copy("one_page.pdf", "cover.pdf");
    let request = || ExportPdfToImagesRequest {
        source_path: source.clone(),
        destination_directory: destination.clone(),
        quality: PdfExportQuality::Standard,
        format: PdfExportFormat::Jpg,
    };

    let first = export_pdf_to_images_with_backend(renderer(), request(), |_| {})
        .expect("first JPG should export");
    let first_bytes = fs::read(&first.output_files[0]).expect("first JPG should be readable");
    let second = export_pdf_to_images_with_backend(renderer(), request(), |_| {})
        .expect("colliding JPG should be numbered");

    assert_eq!(
        first.output_files,
        [destination.join("cover-page-0001.jpg")]
    );
    assert_eq!(
        second.output_files,
        [destination.join("cover-page-0001 (1).jpg")]
    );
    assert_eq!(&first_bytes[..3], b"\xff\xd8\xff");
    assert_eq!(fs::read(&first.output_files[0]).unwrap(), first_bytes);
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
            format: PdfExportFormat::Png,
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
            format: PdfExportFormat::Png,
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
            format: PdfExportFormat::Png,
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
fn existing_multi_page_output_directory_allocates_a_numbered_folder() {
    let (_directory, source, destination) = working_copy("two_page.pdf", "invoice.pdf");
    let existing_directory = destination.join("INVOICE");
    fs::create_dir(&existing_directory).expect("collision directory should be created");
    let existing = existing_directory.join("keep.txt");
    fs::write(&existing, b"existing").expect("collision content should be created");

    let result = export_pdf_to_images_with_backend(
        renderer(),
        ExportPdfToImagesRequest {
            source_path: source,
            destination_directory: destination.clone(),
            quality: PdfExportQuality::Standard,
            format: PdfExportFormat::Png,
        },
        |_| {},
    )
    .expect("a collision should allocate the next folder name");

    assert_eq!(
        result.output_files[0].parent(),
        Some(destination.join("invoice (1)").as_path())
    );
    assert_eq!(
        result.output_files[0].file_name(),
        Some(OsStr::new("invoice-page-0001.png"))
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
fn existing_single_page_output_allocates_a_numbered_file() {
    let (_directory, source, destination) = working_copy("one_page.pdf", "cover.pdf");
    let existing = destination.join("cover-page-0001.png");
    fs::write(&existing, b"existing").expect("collision should be created");

    let result = export_pdf_to_images_with_backend(
        renderer(),
        ExportPdfToImagesRequest {
            source_path: source,
            destination_directory: destination.clone(),
            quality: PdfExportQuality::Standard,
            format: PdfExportFormat::Png,
        },
        |_| {},
    )
    .expect("an existing output should allocate the next file name");

    assert_eq!(
        result.output_files,
        [destination.join("cover-page-0001 (1).png")]
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
            format: PdfExportFormat::Png,
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
            format: PdfExportFormat::Png,
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
            format: PdfExportFormat::Png,
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
    let destination = tempfile::tempdir().expect("destination should be created");
    let plan = output_plan(
        Path::new("invoice.pdf"),
        destination.path(),
        1_000,
        PdfExportFormat::Png,
    )
    .expect("output plan should be valid");

    assert_eq!(
        plan.destinations[0],
        destination
            .path()
            .join("invoice")
            .join("invoice-page-0001.png")
    );
    assert_eq!(
        plan.destinations[9],
        destination
            .path()
            .join("invoice")
            .join("invoice-page-0010.png")
    );
    assert_eq!(
        plan.destinations[999],
        destination
            .path()
            .join("invoice")
            .join("invoice-page-1000.png")
    );
}

#[test]
fn concurrent_folder_allocation_never_reuses_a_name() {
    let destination = tempfile::tempdir().expect("destination should be created");
    let barrier = Arc::new(Barrier::new(2));
    let handles = (0..2)
        .map(|_| {
            let destination = destination.path().to_path_buf();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                create_numbered_output_directory(&destination, OsStr::new("invoice"))
                    .expect("folder allocation should retry safely")
            })
        })
        .collect::<Vec<_>>();
    let mut allocated = handles
        .into_iter()
        .map(|handle| handle.join().expect("allocator should finish"))
        .collect::<Vec<_>>();
    allocated.sort();

    assert_eq!(
        allocated,
        [
            destination.path().join("invoice"),
            destination.path().join("invoice (1)"),
        ]
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

    fn render_page_to_image_at_dpi(
        &self,
        request: PdfDpiRenderRequest,
        format: PdfImageFormat,
        output: &mut (impl Write + Seek),
    ) -> ApplicationResult<RenderedPage> {
        if request.page_index == 1 {
            return Err(ApplicationError::new(
                ApplicationErrorCode::RenderingFailed,
                "The PDF page could not be rendered",
            ));
        }
        self.renderer
            .render_page_to_image_at_dpi(request, format, output)
            .map_err(Into::into)
    }
}
