use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::StructuralPdfPageRangeRequest;
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

    fn merge(
        &self,
        request: &StructuralPdfMergeRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        if self.rewrite_result.is_ok() {
            fs::write(&request.working_output_path, b"merged PDF")
                .map_err(|_| StructuralPdfError::OutputWriteFailed)?;
        }
        self.rewrite_result
    }

    fn create_page_range(
        &self,
        request: &StructuralPdfPageRangeRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        fs::write(&request.working_output_path, b"range")
            .map_err(|_| StructuralPdfError::OutputWriteFailed)?;
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
    fn verify(&self, _path: &Path, _render_representative_page: bool) -> ApplicationResult<u32> {
        self.page_counts
            .lock()
            .expect("fake verifier lock should be available")
            .pop_front()
            .expect("a page count should be configured")
    }
}

struct RecordingMergeEngine {
    validations: Mutex<VecDeque<Result<StructuralPdfValidation, StructuralPdfError>>>,
    merge_result: Result<StructuralPdfOperationResult, StructuralPdfError>,
    merge_requests: Mutex<Vec<StructuralPdfMergeRequest>>,
}

impl RecordingMergeEngine {
    fn with_validations(
        validations: impl IntoIterator<Item = Result<StructuralPdfValidation, StructuralPdfError>>,
    ) -> Self {
        Self {
            validations: Mutex::new(validations.into_iter().collect()),
            merge_result: Ok(StructuralPdfOperationResult {
                has_warnings: false,
            }),
            merge_requests: Mutex::new(Vec::new()),
        }
    }
}

impl StructuralPdfEngine for RecordingMergeEngine {
    fn probe(&self) -> Result<StructuralPdfEngineInfo, StructuralPdfError> {
        Ok(StructuralPdfEngineInfo {
            family: StructuralPdfEngineFamily::Qpdf,
            version: StructuralPdfVersion::new(12, 4, 1),
        })
    }

    fn validate(&self, _source_path: &Path) -> Result<StructuralPdfValidation, StructuralPdfError> {
        self.validations
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Ok(StructuralPdfValidation {
                has_warnings: false,
            }))
    }

    fn rewrite(
        &self,
        _source_path: &Path,
        _working_output_path: &Path,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        Err(StructuralPdfError::OperationFailed)
    }

    fn merge(
        &self,
        request: &StructuralPdfMergeRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        self.merge_requests.lock().unwrap().push(request.clone());
        if self.merge_result.is_ok() {
            fs::write(&request.working_output_path, b"merged PDF")
                .map_err(|_| StructuralPdfError::OutputWriteFailed)?;
        }
        self.merge_result
    }

    fn create_page_range(
        &self,
        _request: &StructuralPdfPageRangeRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        Err(StructuralPdfError::OperationFailed)
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

#[test]
fn merge_requires_two_input_instances() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("only.pdf");
    fs::write(&source, b"source").unwrap();
    let engine = RecordingMergeEngine::with_validations([]);

    let failure = merge_pdf_with_verifier(
        &engine,
        &FakePdfVerifier::with_page_counts([]),
        MergePdfRequest {
            source_paths: vec![source],
            destination_directory: directory.path().to_path_buf(),
            output_name: "merged.pdf".to_owned(),
        },
        |_| {},
    )
    .expect_err("one PDF must not be merged");

    assert_eq!(failure.error.code, ApplicationErrorCode::InvalidRequest);
    assert!(engine.merge_requests.lock().unwrap().is_empty());
}

#[test]
fn merge_preserves_order_duplicates_page_totals_sources_and_progress() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("A.pdf");
    let second = directory.path().join("B.pdf");
    fs::write(&first, b"first source").unwrap();
    fs::write(&second, b"second source").unwrap();
    let before_first = fs::read(&first).unwrap();
    let before_second = fs::read(&second).unwrap();
    let engine = RecordingMergeEngine::with_validations([]);
    let verifier = FakePdfVerifier::with_page_counts([2, 3, 2, 7]);
    let mut stages = Vec::new();

    let result = merge_pdf_with_verifier(
        &engine,
        &verifier,
        MergePdfRequest {
            source_paths: vec![first.clone(), second.clone(), first.clone()],
            destination_directory: directory.path().to_path_buf(),
            output_name: "merged".to_owned(),
        },
        |progress| stages.push(progress),
    )
    .expect("ordered duplicate inputs should merge");

    let requests = engine.merge_requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].ordered_source_paths,
        [first.clone(), second.clone(), first.clone()]
    );
    assert_eq!(result.output_path, directory.path().join("merged.pdf"));
    assert_eq!(result.input_count, 3);
    assert_eq!(result.page_count, 7);
    assert_eq!(fs::read(first).unwrap(), before_first);
    assert_eq!(fs::read(second).unwrap(), before_second);
    assert_eq!(
        stages
            .iter()
            .map(|progress| progress.stage)
            .collect::<Vec<_>>(),
        [
            MergePdfStage::Preparing,
            MergePdfStage::Merging,
            MergePdfStage::Validating,
            MergePdfStage::Publishing,
            MergePdfStage::Completed,
        ]
    );
    assert_eq!(stages[1].total_page_count, 7);
}

#[test]
fn one_invalid_or_password_protected_input_blocks_the_whole_merge() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("A.pdf");
    let second = directory.path().join("B.pdf");
    fs::write(&first, b"first").unwrap();
    fs::write(&second, b"second").unwrap();

    for (engine_error, expected_code) in [
        (
            StructuralPdfError::InvalidDocument,
            ApplicationErrorCode::InvalidPdf,
        ),
        (
            StructuralPdfError::PasswordRequired,
            ApplicationErrorCode::PasswordRequired,
        ),
    ] {
        let engine = RecordingMergeEngine::with_validations([
            Ok(StructuralPdfValidation {
                has_warnings: false,
            }),
            Err(engine_error),
        ]);
        let failure = merge_pdf_with_verifier(
            &engine,
            &FakePdfVerifier::with_page_counts([2]),
            MergePdfRequest {
                source_paths: vec![first.clone(), second.clone()],
                destination_directory: directory.path().to_path_buf(),
                output_name: "failed.pdf".to_owned(),
            },
            |_| {},
        )
        .expect_err("a failed preflight must stop the merge");

        assert_eq!(failure.error.code, expected_code);
        assert_eq!(failure.input_index, Some(1));
        assert_eq!(failure.input_path.as_deref(), Some(second.as_path()));
        assert!(engine.merge_requests.lock().unwrap().is_empty());
        assert!(!directory.path().join("failed.pdf").exists());
    }
}

#[test]
fn recoverable_input_warnings_continue_and_are_counted_per_instance() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("warning.pdf");
    let second = directory.path().join("clean.pdf");
    fs::write(&first, b"first").unwrap();
    fs::write(&second, b"second").unwrap();
    let engine = RecordingMergeEngine::with_validations([
        Ok(StructuralPdfValidation { has_warnings: true }),
        Ok(StructuralPdfValidation {
            has_warnings: false,
        }),
        Ok(StructuralPdfValidation {
            has_warnings: false,
        }),
    ]);

    let result = merge_pdf_with_verifier(
        &engine,
        &FakePdfVerifier::with_page_counts([1, 2, 3]),
        MergePdfRequest {
            source_paths: vec![first, second],
            destination_directory: directory.path().to_path_buf(),
            output_name: "warnings.pdf".to_owned(),
        },
        |_| {},
    )
    .expect("recoverable warnings may merge after output validation");

    assert_eq!(result.warning_input_count, 1);
    assert!(result.has_warnings);
}

#[test]
fn output_page_count_mismatch_prevents_publication() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("A.pdf");
    let second = directory.path().join("B.pdf");
    fs::write(&first, b"first").unwrap();
    fs::write(&second, b"second").unwrap();
    let engine = RecordingMergeEngine::with_validations([]);

    let failure = merge_pdf_with_verifier(
        &engine,
        &FakePdfVerifier::with_page_counts([2, 3, 4]),
        MergePdfRequest {
            source_paths: vec![first, second],
            destination_directory: directory.path().to_path_buf(),
            output_name: "invalid-output.pdf".to_owned(),
        },
        |_| {},
    )
    .expect_err("a page-count mismatch must not publish");

    assert_eq!(
        failure.error.code,
        ApplicationErrorCode::StructuralPdfOutputValidationFailed
    );
    assert!(!directory.path().join("invalid-output.pdf").exists());
    assert_eq!(
        fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry
                .file_name()
                .to_string_lossy()
                .contains(".ilikepdf-merge-pdf-"))
            .count(),
        0
    );
}
