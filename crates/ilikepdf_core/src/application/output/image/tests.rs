use std::io::Write;
use std::sync::{Arc, Barrier};
use std::thread;

use super::*;

#[test]
fn rejects_wrong_preview_extension() {
    let error = PendingImageOutput::create_png(Some(Path::new("preview.jpg")))
        .err()
        .expect("non-PNG preview destinations should be rejected");

    assert_eq!(error.code, ApplicationErrorCode::InvalidRequest);
}

#[test]
fn refuses_to_replace_an_existing_output() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let destination = directory.path().join("existing.png");
    fs::write(&destination, b"existing content").expect("test output should be created");

    let error = PendingImageOutput::create_png(Some(&destination))
        .err()
        .expect("existing output should be rejected");

    assert_eq!(error.code, ApplicationErrorCode::OutputAlreadyExists);
    assert_eq!(
        fs::read(destination).expect("existing output should remain readable"),
        b"existing content"
    );
}

#[test]
fn writable_directory_validation_leaves_no_artifact() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");

    validate_writable_output_directory(directory.path())
        .expect("temporary directory should be writable");

    assert_eq!(
        fs::read_dir(directory.path())
            .expect("temporary directory should remain readable")
            .count(),
        0
    );
}

#[test]
fn numbered_publication_uses_the_lowest_gap_without_replacing_files() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let base = directory.path().join("cover-page-0001.png");
    let second = directory.path().join("cover-page-0001 (2).png");
    fs::write(&base, b"base").expect("base output should be written");
    fs::write(&second, b"second").expect("numbered output should be written");
    let mut pending = PendingImageOutput::for_numbered_destination(&base, "png")
        .expect("temporary PNG should be created");
    pending
        .file
        .write_all(b"new")
        .expect("temporary PNG should be writable");

    let published = pending
        .publish()
        .expect("the lowest numbering gap should be used");

    assert_eq!(published, directory.path().join("cover-page-0001 (1).png"));
    assert_eq!(fs::read(base).unwrap(), b"base");
    assert_eq!(fs::read(second).unwrap(), b"second");
    assert_eq!(fs::read(published).unwrap(), b"new");
}

#[test]
fn numbered_publication_detects_case_insensitive_collisions() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    fs::write(directory.path().join("COVER-PAGE-0001.PNG"), b"existing")
        .expect("case-variant output should be written");
    let desired = directory.path().join("cover-page-0001.png");
    let mut pending = PendingImageOutput::for_numbered_destination(&desired, "png")
        .expect("temporary PNG should be created");
    pending
        .file
        .write_all(b"new")
        .expect("temporary PNG should be writable");

    let published = pending
        .publish()
        .expect("case-insensitive collision should be numbered");

    assert_eq!(published, directory.path().join("cover-page-0001 (1).png"));
    assert_eq!(
        fs::read(directory.path().join("COVER-PAGE-0001.PNG")).unwrap(),
        b"existing"
    );
}

#[test]
fn concurrent_numbered_publication_never_clobbers() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let desired = directory.path().join("page.png");
    let pending = [b"first".as_slice(), b"second".as_slice()]
        .into_iter()
        .map(|bytes| {
            let mut output = PendingImageOutput::for_numbered_destination(&desired, "png")
                .expect("temporary PNG should be created");
            output
                .file
                .write_all(bytes)
                .expect("temporary PNG should be writable");
            output
        })
        .collect::<Vec<_>>();
    let barrier = Arc::new(Barrier::new(pending.len()));
    let handles = pending
        .into_iter()
        .map(|output| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                output.publish().expect("publication should retry safely")
            })
        })
        .collect::<Vec<_>>();
    let mut published = handles
        .into_iter()
        .map(|handle| handle.join().expect("publisher should finish"))
        .collect::<Vec<_>>();
    published.sort();

    let mut expected = vec![desired, directory.path().join("page (1).png")];
    expected.sort();
    assert_eq!(published, expected);
    let mut contents = published
        .iter()
        .map(|path| fs::read(path).expect("published PNG should be readable"))
        .collect::<Vec<_>>();
    contents.sort();
    assert_eq!(contents, [b"first".to_vec(), b"second".to_vec()]);
}

#[test]
fn jpg_numbering_preserves_the_selected_extension() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let desired = directory.path().join("cover-page-0001.jpg");
    fs::write(&desired, b"existing").expect("existing JPG should be written");
    let mut pending = PendingImageOutput::for_numbered_destination(&desired, "jpg")
        .expect("temporary image should be created");
    pending
        .file
        .write_all(b"new")
        .expect("temporary image should be writable");

    let published = pending.publish().expect("JPG should be numbered safely");

    assert_eq!(published, directory.path().join("cover-page-0001 (1).jpg"));
    assert_eq!(fs::read(desired).unwrap(), b"existing");
    assert_eq!(fs::read(published).unwrap(), b"new");
}
