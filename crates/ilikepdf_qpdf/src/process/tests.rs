use std::ffi::OsString;
use std::io::Cursor;

use super::*;

#[test]
fn diagnostic_capture_is_bounded_while_the_stream_remainder_is_drained() {
    let input = vec![b'x'; DIAGNOSTIC_LIMIT + 8192];

    let captured = capture_bounded(Cursor::new(input)).expect("capture should succeed");

    assert_eq!(captured.bytes.len(), DIAGNOSTIC_LIMIT);
    assert!(captured.truncated);
}

#[test]
fn an_unlaunchable_runtime_is_reported_without_shell_fallback() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let not_an_executable = directory.path().join("missing-qpdf.exe");

    let error = QpdfProcessRunner
        .run(&not_an_executable, &[OsString::from("--version")], None)
        .expect_err("invalid executable should not launch");

    assert_eq!(error, QpdfProcessError::Launch);
}
