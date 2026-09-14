use std::fs;
use std::io::Write;

use super::*;

#[test]
fn publishing_uses_the_lowest_available_name_without_replacing_files() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let base = directory.path().join("scan.pdf");
    let second = directory.path().join("scan (2).pdf");
    fs::write(&base, b"base").expect("base fixture should be written");
    fs::write(&second, b"second").expect("second fixture should be written");

    let mut pending = PendingPdfOutput::in_directory(directory.path())
        .expect("temporary output should be created");
    pending
        .file
        .write_all(b"new")
        .expect("temporary PDF should be writable");
    let published = pending
        .publish_with_stem(OsStr::new("scan"))
        .expect("the first numbering gap should be used");

    assert_eq!(published, directory.path().join("scan (1).pdf"));
    assert_eq!(fs::read(base).expect("base remains"), b"base");
    assert_eq!(fs::read(second).expect("second remains"), b"second");
    assert_eq!(fs::read(published).expect("new PDF is readable"), b"new");
}

#[test]
fn filename_allocation_is_case_insensitive() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    fs::write(directory.path().join("SCAN.PDF"), b"existing")
        .expect("case variant should be written");

    let mut pending = PendingPdfOutput::in_directory(directory.path())
        .expect("temporary output should be created");
    pending
        .file
        .write_all(b"new")
        .expect("temporary PDF should be writable");
    let published = pending
        .publish_with_stem(OsStr::new("scan"))
        .expect("case-insensitive collision should be numbered");

    assert_eq!(published, directory.path().join("scan (1).pdf"));
    assert_eq!(
        fs::read(directory.path().join("SCAN.PDF")).unwrap(),
        b"existing"
    );
}

#[test]
fn exact_publication_never_replaces_an_existing_pdf() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let destination = directory.path().join("rewritten.pdf");
    fs::write(&destination, b"existing").expect("existing output should be written");

    let error = PendingExactPdfOutput::for_destination(&destination)
        .expect_err("an existing exact destination must be rejected");

    assert_eq!(error.code, ApplicationErrorCode::OutputAlreadyExists);
    assert_eq!(fs::read(destination).unwrap(), b"existing");
}

#[test]
fn exact_publication_uses_a_private_temporary_file() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let destination = directory.path().join("rewritten.pdf");
    let pending = PendingExactPdfOutput::for_destination(&destination)
        .expect("temporary output should be created");
    let working_path = pending.working_path().to_path_buf();
    fs::write(&working_path, b"rewritten").expect("working output should be writable");

    let published = pending.publish().expect("output should publish atomically");

    assert_eq!(published, destination);
    assert!(!working_path.exists());
    assert_eq!(fs::read(published).unwrap(), b"rewritten");
}

#[test]
fn exact_publication_rejects_invalid_destination_inputs() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let wrong_extension = directory.path().join("rewritten.png");
    let missing_directory = directory.path().join("missing").join("rewritten.pdf");

    assert_eq!(
        PendingExactPdfOutput::for_destination(&wrong_extension)
            .expect_err("a structural output must be a PDF")
            .code,
        ApplicationErrorCode::InvalidRequest
    );
    assert_eq!(
        PendingExactPdfOutput::for_destination(&missing_directory)
            .expect_err("the destination directory must exist")
            .code,
        ApplicationErrorCode::InvalidOutputDirectory
    );
}

#[test]
fn pdf_output_io_failures_map_to_stable_application_errors() {
    assert_eq!(
        map_output_io_error(std::io::Error::from(std::io::ErrorKind::PermissionDenied)).code,
        ApplicationErrorCode::PermissionDenied
    );
    assert_eq!(
        map_output_io_error(std::io::Error::from(std::io::ErrorKind::WriteZero)).code,
        ApplicationErrorCode::OutputWriteFailed
    );
}

#[test]
fn merge_output_name_is_a_filename_and_pdf_extension_is_normalized() {
    assert_eq!(normalize_pdf_filename("report").unwrap(), "report.pdf");
    assert_eq!(normalize_pdf_filename("report.pdf").unwrap(), "report.pdf");

    for invalid in [
        "../report.pdf",
        r"..\report.pdf",
        r"C:\report.pdf",
        "foo/bar.pdf",
        "CON.pdf",
        "report. ",
    ] {
        assert_eq!(
            normalize_pdf_filename(invalid).unwrap_err().code,
            ApplicationErrorCode::InvalidRequest
        );
    }
}

#[test]
fn numbered_merge_publication_uses_the_lowest_gap_and_never_clobbers() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let base = directory.path().join("merged.pdf");
    let second = directory.path().join("merged (2).pdf");
    fs::write(&base, b"base").unwrap();
    fs::write(&second, b"second").unwrap();
    let pending = PendingNumberedPdfOutput::in_directory(directory.path(), "merged")
        .expect("merge output should be prepared");
    fs::write(pending.working_path(), b"merged output").unwrap();

    let output = pending.publish().expect("merge output should publish");

    assert_eq!(output, directory.path().join("merged (1).pdf"));
    assert_eq!(fs::read(base).unwrap(), b"base");
    assert_eq!(fs::read(second).unwrap(), b"second");
    assert_eq!(fs::read(output).unwrap(), b"merged output");
}

#[test]
fn merge_collision_matching_is_case_insensitive() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("REPORT.PDF"), b"existing").unwrap();
    let pending = PendingNumberedPdfOutput::in_directory(directory.path(), "report.pdf").unwrap();
    fs::write(pending.working_path(), b"new").unwrap();

    let output = pending.publish().unwrap();

    assert_eq!(output, directory.path().join("report (1).pdf"));
    assert_eq!(
        fs::read(directory.path().join("REPORT.PDF")).unwrap(),
        b"existing"
    );
}
