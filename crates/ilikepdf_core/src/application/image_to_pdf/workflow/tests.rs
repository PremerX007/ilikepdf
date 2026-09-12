use ilikepdf_pdf::PdfRenderer;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use super::super::{ImagePdfMargin, ImagePdfOrientation, ImagePdfPageSize};
use super::*;
use crate::ApplicationErrorCode;

fn request(paths: Vec<PathBuf>, merge: bool) -> CreateImagePdfRequest {
    CreateImagePdfRequest {
        source_paths: paths,
        destination_directory: PathBuf::from("output"),
        page_size: ImagePdfPageSize::A4,
        orientation: ImagePdfOrientation::Portrait,
        margin: ImagePdfMargin::None,
        merge,
    }
}

#[test]
fn naming_uses_the_first_image_in_final_order() {
    let single = output_stems(&request(vec![PathBuf::from("passport.jpg")], true))
        .expect("single output stem");
    let multiple = output_stems(&request(
        vec![PathBuf::from("01.jpg"), PathBuf::from("02.png")],
        true,
    ))
    .expect("merged output stem");
    let reordered = output_stems(&request(
        vec![PathBuf::from("02.png"), PathBuf::from("01.jpg")],
        true,
    ))
    .expect("reordered output stem");
    let separate = output_stems(&request(
        vec![PathBuf::from("01.jpg"), PathBuf::from("02.png")],
        false,
    ))
    .expect("separate output stems");

    assert_eq!(single, [OsString::from("passport")]);
    assert_eq!(multiple, [OsString::from("01")]);
    assert_eq!(reordered, [OsString::from("02")]);
    assert_eq!(separate, [OsString::from("01"), OsString::from("02")]);
}

#[test]
fn duplicate_unmerged_stems_are_kept_in_conversion_order() {
    let stems = output_stems(&request(
        vec![PathBuf::from("A/scan.jpg"), PathBuf::from("B/SCAN.png")],
        false,
    ))
    .expect("duplicate stems are numbered during publication");

    assert_eq!(stems, [OsString::from("scan"), OsString::from("SCAN")]);
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("ilikepdf_pdf")
        .join("tests")
        .join("fixtures")
        .join("images")
        .join(name)
}

fn renderer() -> &'static PdfRenderer {
    crate::test_support::pdf_renderer()
}

fn real_request(
    source_paths: Vec<PathBuf>,
    destination_directory: PathBuf,
    merge: bool,
) -> CreateImagePdfRequest {
    CreateImagePdfRequest {
        source_paths,
        destination_directory,
        page_size: ImagePdfPageSize::A4,
        orientation: ImagePdfOrientation::Portrait,
        margin: ImagePdfMargin::None,
        merge,
    }
}

#[test]
fn merged_and_unmerged_outputs_publish_with_expected_names_and_page_counts() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let merged_destination = directory.path().join("merged");
    let separate_destination = directory.path().join("separate");
    fs::create_dir(&merged_destination).expect("merged directory should be created");
    fs::create_dir(&separate_destination).expect("separate directory should be created");
    let sources = vec![
        fixture("portrait.png"),
        fixture("photo.jpg"),
        fixture("sample.webp"),
    ];
    let source_bytes = sources
        .iter()
        .map(|path| fs::read(path).expect("fixture should be readable"))
        .collect::<Vec<_>>();

    let mut merged_progress = Vec::new();
    let merged = create_pdfs_from_images_with_backend(
        renderer(),
        real_request(sources.clone(), merged_destination.clone(), true),
        |progress| merged_progress.push(progress),
    )
    .expect("merged PDF should succeed");
    assert_eq!(
        merged.output_files,
        [merged_destination.join("portrait.pdf")]
    );
    assert_eq!(
        renderer()
            .inspect_document(&merged.output_files[0])
            .expect("merged PDF should reopen")
            .page_count,
        3
    );
    assert_eq!(
        merged_progress
            .last()
            .expect("progress should be reported")
            .completed_image_count,
        3
    );

    let separate = create_pdfs_from_images_with_backend(
        renderer(),
        real_request(sources.clone(), separate_destination.clone(), false),
        |_| {},
    )
    .expect("separate PDFs should succeed");
    assert_eq!(
        separate.output_files,
        [
            separate_destination.join("portrait.pdf"),
            separate_destination.join("photo.pdf"),
            separate_destination.join("sample.pdf"),
        ]
    );
    for output in &separate.output_files {
        assert_eq!(
            renderer()
                .inspect_document(output)
                .expect("separate PDF should reopen")
                .page_count,
            1
        );
    }

    for (source, expected) in sources.iter().zip(source_bytes) {
        assert_eq!(
            fs::read(source).expect("source should remain readable"),
            expected
        );
    }
}

#[test]
fn merged_collisions_use_the_lowest_gap_and_never_clobber() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let base = directory.path().join("portrait.pdf");
    let second = directory.path().join("portrait (2).pdf");
    fs::write(&base, b"existing base bytes").expect("base fixture should be written");
    fs::write(&second, b"existing second bytes").expect("numbered fixture should be written");

    let result = create_pdfs_from_images_with_backend(
        renderer(),
        real_request(
            vec![fixture("portrait.png")],
            directory.path().to_path_buf(),
            true,
        ),
        |_| {},
    )
    .expect("ordinary collisions should be numbered");

    assert_eq!(
        result.output_files,
        [directory.path().join("portrait (1).pdf")]
    );
    assert_eq!(
        fs::read(base).expect("base output should remain readable"),
        b"existing base bytes"
    );
    assert_eq!(
        fs::read(second).expect("numbered output should remain readable"),
        b"existing second bytes"
    );
}

#[test]
fn unmerged_collisions_and_duplicate_stems_are_numbered_in_order() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source_one = directory.path().join("one");
    let source_two = directory.path().join("two");
    let destination = directory.path().join("output");
    fs::create_dir(&source_one).expect("first source directory should be created");
    fs::create_dir(&source_two).expect("second source directory should be created");
    fs::create_dir(&destination).expect("output directory should be created");
    let first_scan = source_one.join("scan.jpg");
    let second_scan = source_two.join("scan.png");
    fs::copy(fixture("photo.jpg"), &first_scan).expect("JPEG fixture should be copied");
    fs::copy(fixture("portrait.png"), &second_scan).expect("PNG fixture should be copied");
    let existing = destination.join("scan.pdf");
    fs::write(&existing, b"existing PDF bytes").expect("existing output should be written");

    let result = create_pdfs_from_images_with_backend(
        renderer(),
        real_request(vec![first_scan, second_scan], destination.clone(), false),
        |_| {},
    )
    .expect("duplicate stems should receive distinct output names");

    assert_eq!(
        result.output_files,
        [
            destination.join("scan (1).pdf"),
            destination.join("scan (2).pdf")
        ]
    );
    assert_eq!(
        fs::read(existing).expect("existing output should remain readable"),
        b"existing PDF bytes"
    );
}

#[test]
fn unmerged_validation_fails_before_any_output_is_published() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let failure = create_pdfs_from_images_with_backend(
        renderer(),
        real_request(
            vec![fixture("portrait.png"), fixture("malformed.png")],
            directory.path().to_path_buf(),
            false,
        ),
        |_| {},
    )
    .expect_err("malformed input should fail preflight");

    assert!(matches!(
        failure.error.code,
        ApplicationErrorCode::MalformedImage | ApplicationErrorCode::ImageDecodeFailed
    ));
    assert!(failure.output_files.is_empty());
    assert!(!directory.path().join("portrait.pdf").exists());
}
