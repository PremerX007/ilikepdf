use std::fs;

use ilikepdf_pdf::PdfRenderer;

use super::super::model::{PdfExportFormat, PdfExportQuality};
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
    crate::test_support::pdf_renderer()
}

#[test]
fn mixed_batch_continues_in_visual_order_and_reports_overall_progress() {
    let sources = tempfile::tempdir().expect("source directory should be created");
    let destination = tempfile::tempdir().expect("destination should be created");
    let cover = sources.path().join("cover.pdf");
    let broken = sources.path().join("broken.pdf");
    let report = sources.path().join("report.pdf");
    fs::copy(fixture("one_page.pdf"), &cover).expect("cover should be copied");
    fs::copy(fixture("malformed.pdf"), &broken).expect("broken PDF should be copied");
    fs::copy(fixture("two_page.pdf"), &report).expect("report should be copied");
    let source_bytes = [
        fs::read(&cover).expect("cover should be readable"),
        fs::read(&broken).expect("broken PDF should be readable"),
        fs::read(&report).expect("report should be readable"),
    ];
    let mut progress = Vec::new();

    let result = export_pdf_batch_to_images_with_backend(
        renderer(),
        ExportPdfBatchRequest {
            source_paths: vec![cover.clone(), broken.clone(), report.clone()],
            destination_mode: PdfBatchDestinationMode::CustomFolder,
            custom_destination_directory: Some(destination.path().to_path_buf()),
            quality: PdfExportQuality::Standard,
            format: PdfExportFormat::Png,
        },
        |update| progress.push(update),
    )
    .expect("one malformed PDF should not stop later documents");

    assert_eq!(result.total_document_count, 3);
    assert_eq!(result.succeeded_document_count, 2);
    assert_eq!(result.failed_document_count, 1);
    assert_eq!(result.total_page_count, 3);
    assert_eq!(result.completed_page_count, 3);
    assert_eq!(
        result
            .documents
            .iter()
            .map(|document| document.display_name.as_str())
            .collect::<Vec<_>>(),
        ["cover.pdf", "broken.pdf", "report.pdf"]
    );
    assert_eq!(
        result.documents[1]
            .error
            .as_ref()
            .expect("broken PDF should fail")
            .code,
        ApplicationErrorCode::InvalidPdf
    );
    assert_eq!(
        result.documents[0].output_files,
        [destination.path().join("cover-page-0001.png")]
    );
    assert_eq!(
        result.documents[2].output_files[0].parent(),
        Some(destination.path().join("report").as_path())
    );
    assert_eq!(
        progress
            .last()
            .expect("batch should report final progress")
            .completed_page_count,
        3
    );
    assert_eq!(fs::read(cover).unwrap(), source_bytes[0]);
    assert_eq!(fs::read(broken).unwrap(), source_bytes[1]);
    assert_eq!(fs::read(report).unwrap(), source_bytes[2]);
}

#[test]
fn next_to_source_files_uses_each_documents_own_directory() {
    let first_directory = tempfile::tempdir().expect("first directory should be created");
    let second_directory = tempfile::tempdir().expect("second directory should be created");
    let cover = first_directory.path().join("cover.pdf");
    let report = second_directory.path().join("report.pdf");
    fs::copy(fixture("one_page.pdf"), &cover).expect("cover should be copied");
    fs::copy(fixture("two_page.pdf"), &report).expect("report should be copied");

    let result = export_pdf_batch_to_images_with_backend(
        renderer(),
        ExportPdfBatchRequest {
            source_paths: vec![cover, report],
            destination_mode: PdfBatchDestinationMode::NextToSourceFiles,
            custom_destination_directory: None,
            quality: PdfExportQuality::Standard,
            format: PdfExportFormat::Png,
        },
        |_| {},
    )
    .expect("both PDFs should export beside themselves");

    assert_eq!(
        result.documents[0].output_files,
        [first_directory.path().join("cover-page-0001.png")]
    );
    assert_eq!(
        result.documents[1].output_files[0].parent(),
        Some(second_directory.path().join("report").as_path())
    );
}

#[test]
fn invalid_custom_destination_stops_before_processing_documents() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let invalid_destination = directory.path().join("not-a-directory");
    fs::write(&invalid_destination, b"occupied")
        .expect("invalid destination fixture should be created");

    let failure = export_pdf_batch_to_images_with_backend(
        renderer(),
        ExportPdfBatchRequest {
            source_paths: vec![fixture("one_page.pdf")],
            destination_mode: PdfBatchDestinationMode::CustomFolder,
            custom_destination_directory: Some(invalid_destination),
            quality: PdfExportQuality::Standard,
            format: PdfExportFormat::Png,
        },
        |_| panic!("global validation failure must not report document progress"),
    )
    .expect_err("invalid custom destination should stop the batch");

    assert_eq!(failure.completed_document_count, 0);
    assert!(failure.documents.is_empty());
    assert_eq!(
        failure.error.code,
        ApplicationErrorCode::InvalidOutputDirectory
    );
}

#[test]
fn repeated_batch_uses_numbered_files_and_folders_without_clobbering() {
    let sources = tempfile::tempdir().expect("source directory should be created");
    let destination = tempfile::tempdir().expect("destination should be created");
    let cover = sources.path().join("cover.pdf");
    let report = sources.path().join("report.pdf");
    fs::copy(fixture("one_page.pdf"), &cover).expect("cover should be copied");
    fs::copy(fixture("two_page.pdf"), &report).expect("report should be copied");
    let request = || ExportPdfBatchRequest {
        source_paths: vec![cover.clone(), report.clone()],
        destination_mode: PdfBatchDestinationMode::CustomFolder,
        custom_destination_directory: Some(destination.path().to_path_buf()),
        quality: PdfExportQuality::Standard,
        format: PdfExportFormat::Png,
    };

    let first = export_pdf_batch_to_images_with_backend(renderer(), request(), |_| {})
        .expect("first batch should export");
    let first_png_bytes = fs::read(&first.documents[0].output_files[0]).unwrap();
    let second = export_pdf_batch_to_images_with_backend(renderer(), request(), |_| {})
        .expect("second batch should be numbered");
    let third = export_pdf_batch_to_images_with_backend(renderer(), request(), |_| {})
        .expect("third batch should be numbered");

    assert_eq!(
        second.documents[0].output_files,
        [destination.path().join("cover-page-0001 (1).png")]
    );
    assert_eq!(
        third.documents[0].output_files,
        [destination.path().join("cover-page-0001 (2).png")]
    );
    assert_eq!(
        second.documents[1].output_files[0].parent(),
        Some(destination.path().join("report (1)").as_path())
    );
    assert_eq!(
        third.documents[1].output_files[0].parent(),
        Some(destination.path().join("report (2)").as_path())
    );
    assert_eq!(
        second.documents[1].output_files[0].file_name(),
        Some(std::ffi::OsStr::new("report-page-0001.png"))
    );
    assert_eq!(
        fs::read(&first.documents[0].output_files[0]).unwrap(),
        first_png_bytes
    );
}
