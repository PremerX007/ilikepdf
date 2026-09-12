use super::*;

#[test]
fn missing_runtime_is_a_structured_error() {
    let error = load(Path::new("definitely-missing-pdfium.dll"))
        .expect_err("a missing runtime must not bind");

    assert_eq!(error.kind, PdfErrorKind::RuntimeUnavailable);
}
