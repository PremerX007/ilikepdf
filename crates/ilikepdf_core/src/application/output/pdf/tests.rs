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
