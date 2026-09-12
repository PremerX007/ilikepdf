use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::{StructuralPdfEngineFamily, StructuralPdfOperationResult, StructuralPdfVersion};

struct FakeStructuralPdfEngine {
    validate_result: Result<StructuralPdfValidation, StructuralPdfError>,
    rewrite_result: Result<StructuralPdfOperationResult, StructuralPdfError>,
    rewrite_calls: AtomicUsize,
}

impl FakeStructuralPdfEngine {
    fn successful() -> Self {
        Self {
            validate_result: Ok(StructuralPdfValidation {
                has_warnings: false,
            }),
            rewrite_result: Ok(StructuralPdfOperationResult {
                has_warnings: false,
            }),
            rewrite_calls: AtomicUsize::new(0),
        }
    }
}

impl StructuralPdfEngine for FakeStructuralPdfEngine {
    fn probe(&self) -> Result<StructuralPdfEngineInfo, StructuralPdfError> {
        Ok(StructuralPdfEngineInfo {
            family: StructuralPdfEngineFamily::Qpdf,
            version: StructuralPdfVersion::new(12, 4, 1),
        })
    }

    fn validate(&self, _source_path: &Path) -> Result<StructuralPdfValidation, StructuralPdfError> {
        self.validate_result
    }

    fn rewrite(
        &self,
        _source_path: &Path,
        working_output_path: &Path,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        self.rewrite_calls.fetch_add(1, Ordering::SeqCst);
        if self.rewrite_result.is_ok() {
            fs::write(working_output_path, b"rewritten PDF")
                .map_err(|_| StructuralPdfError::OutputWriteFailed)?;
        }
        self.rewrite_result
    }
}

struct FakePdfVerifier {
    page_counts: Mutex<VecDeque<ApplicationResult<u32>>>,
}

impl FakePdfVerifier {
    fn with_page_counts(page_counts: impl IntoIterator<Item = u32>) -> Self {
        Self {
            page_counts: Mutex::new(page_counts.into_iter().map(Ok).collect()),
        }
    }
}

impl PdfVerifier for FakePdfVerifier {
    fn verify(&self, _path: &Path) -> ApplicationResult<u32> {
        self.page_counts
            .lock()
            .expect("fake verifier lock should be available")
            .pop_front()
            .expect("a page count should be configured")
    }
}

#[test]
fn application_core_uses_a_process_neutral_fake_engine() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = directory.path().join("source.pdf");
    fs::write(&source, b"fake PDF").expect("source should be written");
    let engine = FakeStructuralPdfEngine::successful();

    let info = probe_structural_pdf_engine(&engine).expect("probe should succeed");
    let validation = validate_structural_pdf(&engine, &source).expect("validation should succeed");

    assert_eq!(info.family, StructuralPdfEngineFamily::Qpdf);
    assert_eq!(info.version, StructuralPdfVersion::new(12, 4, 1));
    assert!(!validation.has_warnings);
}

#[test]
fn rewrite_preserves_the_source_and_publishes_only_after_verification() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = directory.path().join("source.pdf");
    let destination = directory.path().join("rewritten.pdf");
    fs::write(&source, b"original PDF").expect("source should be written");
    let original = fs::read(&source).unwrap();
    let engine = FakeStructuralPdfEngine::successful();
    let verifier = FakePdfVerifier::with_page_counts([2, 2]);

    let result = rewrite_structural_pdf_with_verifier(
        &engine,
        &verifier,
        RewriteStructuralPdfRequest {
            source_path: source.clone(),
            destination_path: destination.clone(),
        },
    )
    .expect("rewrite should succeed");

    assert_eq!(result.output_path, destination);
    assert_eq!(result.page_count, 2);
    assert_eq!(fs::read(source).unwrap(), original);
    assert_eq!(fs::read(result.output_path).unwrap(), b"rewritten PDF");
}

#[test]
fn output_collision_stops_before_the_engine_writes() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = directory.path().join("source.pdf");
    let destination = directory.path().join("existing.pdf");
    fs::write(&source, b"source").unwrap();
    fs::write(&destination, b"existing").unwrap();
    let engine = FakeStructuralPdfEngine::successful();
    let verifier = FakePdfVerifier::with_page_counts([]);

    let error = rewrite_structural_pdf_with_verifier(
        &engine,
        &verifier,
        RewriteStructuralPdfRequest {
            source_path: source,
            destination_path: destination.clone(),
        },
    )
    .expect_err("existing output must not be replaced");

    assert_eq!(error.code, ApplicationErrorCode::OutputAlreadyExists);
    assert_eq!(engine.rewrite_calls.load(Ordering::SeqCst), 0);
    assert_eq!(fs::read(destination).unwrap(), b"existing");
}

#[test]
fn failed_output_verification_removes_the_private_working_file() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = directory.path().join("source.pdf");
    let destination = directory.path().join("rewritten.pdf");
    fs::write(&source, b"source").unwrap();
    let engine = FakeStructuralPdfEngine::successful();
    let verifier = FakePdfVerifier::with_page_counts([2, 1]);

    let error = rewrite_structural_pdf_with_verifier(
        &engine,
        &verifier,
        RewriteStructuralPdfRequest {
            source_path: source,
            destination_path: destination.clone(),
        },
    )
    .expect_err("mismatched page counts must fail");

    assert_eq!(
        error.code,
        ApplicationErrorCode::StructuralPdfOutputValidationFailed
    );
    assert!(!destination.exists());
    assert_eq!(
        fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry
                .file_name()
                .to_string_lossy()
                .contains("structural-pdf"))
            .count(),
        0
    );
}

#[test]
fn engine_failures_map_to_safe_application_errors() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = directory.path().join("source.pdf");
    fs::write(&source, b"source").unwrap();
    let engine = FakeStructuralPdfEngine {
        validate_result: Err(StructuralPdfError::OperationFailed),
        rewrite_result: Ok(StructuralPdfOperationResult {
            has_warnings: false,
        }),
        rewrite_calls: AtomicUsize::new(0),
    };

    let error = validate_structural_pdf(&engine, &source)
        .expect_err("engine operation failure should be mapped");

    assert_eq!(
        error.code,
        ApplicationErrorCode::StructuralPdfOperationFailed
    );
    assert_eq!(error.message, "The structural PDF operation failed");
    assert!(!error.message.contains("stderr"));
}
