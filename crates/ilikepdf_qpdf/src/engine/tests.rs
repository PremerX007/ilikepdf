use std::collections::VecDeque;
use std::sync::Mutex;

use crate::manifest::runtime_manifest;
use crate::process::BoundedDiagnostic;

use super::*;

struct FakeRunner {
    outputs: Mutex<VecDeque<Result<QpdfProcessOutput, QpdfProcessError>>>,
}

struct RecordingMergeRunner {
    calls: Mutex<Vec<Vec<OsString>>>,
}

impl QpdfRunner for RecordingMergeRunner {
    fn run(
        &self,
        _executable: &Path,
        arguments: &[OsString],
        _stdin_data: Option<&[u8]>,
    ) -> Result<QpdfProcessOutput, QpdfProcessError> {
        self.calls.lock().unwrap().push(arguments.to_vec());
        if arguments == [OsString::from("--version")] {
            return Ok(QpdfProcessOutput {
                exit_code: Some(0),
                stdout: diagnostic(b"qpdf version 12.4.1\n"),
                stderr: diagnostic(b""),
            });
        }
        fs::write(PathBuf::from(arguments.last().unwrap()), b"merged").unwrap();
        Ok(QpdfProcessOutput {
            exit_code: Some(0),
            stdout: diagnostic(b""),
            stderr: diagnostic(b""),
        })
    }
}

impl QpdfRunner for FakeRunner {
    fn run(
        &self,
        _executable: &Path,
        _arguments: &[OsString],
        _stdin_data: Option<&[u8]>,
    ) -> Result<QpdfProcessOutput, QpdfProcessError> {
        self.outputs
            .lock()
            .unwrap()
            .pop_front()
            .expect("a fake result should be configured")
    }
}

fn diagnostic(value: &[u8]) -> BoundedDiagnostic {
    BoundedDiagnostic::for_test(value)
}

#[test]
fn version_parser_returns_typed_components() {
    assert_eq!(
        parse_version("qpdf version 12.4.1\r\nmore information"),
        Ok(StructuralPdfVersion::new(12, 4, 1))
    );
    assert_eq!(
        parse_version("unexpected output"),
        Err(StructuralPdfError::RuntimeIncompatible)
    );
}

#[test]
fn simulated_nonzero_probe_does_not_expose_raw_process_diagnostics() {
    let runtime_root = fixture_runtime_root();
    let runner = FakeRunner {
        outputs: Mutex::new(VecDeque::from([Ok(QpdfProcessOutput {
            exit_code: Some(2),
            stdout: diagnostic(b""),
            stderr: diagnostic(b"secret raw stderr and file path"),
        })])),
    };
    let engine = QpdfCliEngine::with_runner(
        QpdfRuntimeResolver::from_root(runtime_root.path()),
        Arc::new(runner),
    );

    let error = engine.probe().expect_err("nonzero probe should fail");

    assert_eq!(error, StructuralPdfError::RuntimeIncompatible);
}

#[test]
fn simulated_launch_failure_maps_to_a_stable_runtime_error() {
    let runtime_root = fixture_runtime_root();
    let runner = FakeRunner {
        outputs: Mutex::new(VecDeque::from([Err(QpdfProcessError::Launch)])),
    };
    let engine = QpdfCliEngine::with_runner(
        QpdfRuntimeResolver::from_root(runtime_root.path()),
        Arc::new(runner),
    );

    assert_eq!(engine.probe(), Err(StructuralPdfError::RuntimeLaunchFailed));
}

#[test]
fn simulated_nonzero_validation_maps_to_a_domain_error_without_raw_details() {
    let runtime_root = fixture_runtime_root();
    let source = runtime_root.path().join("document.pdf");
    fs::write(&source, b"fixture").unwrap();
    let runner = FakeRunner {
        outputs: Mutex::new(VecDeque::from([
            Ok(QpdfProcessOutput {
                exit_code: Some(0),
                stdout: diagnostic(b"qpdf version 12.4.1\n"),
                stderr: diagnostic(b""),
            }),
            Ok(QpdfProcessOutput {
                exit_code: Some(2),
                stdout: diagnostic(b""),
                stderr: diagnostic(b""),
            }),
            Ok(QpdfProcessOutput {
                exit_code: Some(2),
                stdout: diagnostic(b""),
                stderr: diagnostic(b"raw qpdf diagnostics must remain internal"),
            }),
        ])),
    };
    let engine = QpdfCliEngine::with_runner(
        QpdfRuntimeResolver::from_root(runtime_root.path()),
        Arc::new(runner),
    );

    assert_eq!(
        engine.validate(&source),
        Err(StructuralPdfError::InvalidDocument)
    );
}

#[test]
fn password_required_is_classified_without_exposing_diagnostics() {
    let runtime_root = fixture_runtime_root();
    let source = runtime_root.path().join("protected.pdf");
    fs::write(&source, b"fixture").unwrap();
    let runner = FakeRunner {
        outputs: Mutex::new(VecDeque::from([
            Ok(QpdfProcessOutput {
                exit_code: Some(0),
                stdout: diagnostic(b"qpdf version 12.4.1\n"),
                stderr: diagnostic(b""),
            }),
            Ok(QpdfProcessOutput {
                exit_code: Some(0),
                stdout: diagnostic(b""),
                stderr: diagnostic(b"private password diagnostics"),
            }),
        ])),
    };
    let engine = QpdfCliEngine::with_runner(
        QpdfRuntimeResolver::from_root(runtime_root.path()),
        Arc::new(runner),
    );

    assert_eq!(
        engine.validate(&source),
        Err(StructuralPdfError::PasswordRequired)
    );
}

#[test]
fn recoverable_validation_exit_is_a_typed_warning() {
    let runtime_root = fixture_runtime_root();
    let source = runtime_root.path().join("warning.pdf");
    fs::write(&source, b"fixture").unwrap();
    let runner = FakeRunner {
        outputs: Mutex::new(VecDeque::from([
            Ok(QpdfProcessOutput {
                exit_code: Some(0),
                stdout: diagnostic(b"qpdf version 12.4.1\n"),
                stderr: diagnostic(b""),
            }),
            Ok(QpdfProcessOutput {
                exit_code: Some(2),
                stdout: diagnostic(b""),
                stderr: diagnostic(b""),
            }),
            Ok(QpdfProcessOutput {
                exit_code: Some(3),
                stdout: diagnostic(b""),
                stderr: diagnostic(b"recoverable warning details stay private"),
            }),
        ])),
    };
    let engine = QpdfCliEngine::with_runner(
        QpdfRuntimeResolver::from_root(runtime_root.path()),
        Arc::new(runner),
    );

    assert_eq!(
        engine.validate(&source),
        Ok(StructuralPdfValidation { has_warnings: true })
    );
}

#[test]
fn real_password_protected_fixture_is_created_and_classified_in_test_setup() {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let runtime_root = repository_root
        .join("third_party")
        .join("qpdf")
        .join("windows")
        .join("x64");
    let executable = runtime_root.join("bin").join("qpdf.exe");
    let source = repository_root
        .join("crates")
        .join("ilikepdf_pdf")
        .join("tests")
        .join("fixtures")
        .join("one_page.pdf");
    let directory = tempfile::tempdir().unwrap();
    let protected = directory.path().join("password protected.pdf");
    let creation = QpdfProcessRunner
        .run(
            &executable,
            &[
                source.into_os_string(),
                OsString::from("--encrypt"),
                OsString::from("--user-password=merge-test-password"),
                OsString::from("--owner-password=merge-test-owner"),
                OsString::from("--bits=256"),
                OsString::from("--"),
                protected.clone().into_os_string(),
            ],
            None,
        )
        .expect("test fixture encryption should launch");
    assert_eq!(creation.exit_code, Some(0));
    let engine = QpdfCliEngine::from_runtime_root(runtime_root);

    assert_eq!(
        engine.validate(&protected),
        Err(StructuralPdfError::PasswordRequired)
    );
    let failure = ilikepdf_core::split_pdf(
        &engine,
        ilikepdf_core::SplitPdfRequest {
            source_path: protected,
            destination_directory: directory.path().to_path_buf(),
            mode: ilikepdf_core::SplitPdfMode::EveryPage,
        },
        |_| {},
    )
    .expect_err("password-protected input must block Split before publication");
    assert_eq!(
        failure.error.code,
        ilikepdf_core::ApplicationErrorCode::PasswordRequired
    );
    assert!(!directory.path().join("password protected-split").exists());
}

#[test]
fn merge_maps_ordered_duplicate_sources_to_qpdf_page_composition() {
    let runtime_root = fixture_runtime_root();
    let first = runtime_root.path().join("first source.pdf");
    let second = runtime_root.path().join("second.pdf");
    let output = runtime_root.path().join("working.tmp");
    fs::write(&first, b"first").unwrap();
    fs::write(&second, b"second").unwrap();
    fs::write(&output, b"").unwrap();
    let runner = Arc::new(RecordingMergeRunner {
        calls: Mutex::new(Vec::new()),
    });
    let engine = QpdfCliEngine::with_runner(
        QpdfRuntimeResolver::from_root(runtime_root.path()),
        runner.clone(),
    );

    engine
        .merge(&StructuralPdfMergeRequest {
            ordered_source_paths: vec![first.clone(), second.clone(), first.clone()],
            working_output_path: output.clone(),
        })
        .expect("merge should succeed");

    let calls = runner.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1][0], "--empty");
    assert_eq!(calls[1][1], "--pages");
    assert_eq!(
        PathBuf::from(&calls[1][2]),
        std::path::absolute(first).unwrap()
    );
    assert_eq!(
        PathBuf::from(&calls[1][3]),
        std::path::absolute(second).unwrap()
    );
    assert_eq!(calls[1][2], calls[1][4]);
    assert_eq!(calls[1][5], "--");
    assert_eq!(
        PathBuf::from(&calls[1][6]),
        std::path::absolute(output).unwrap()
    );
}

#[test]
fn page_range_maps_an_inclusive_contiguous_range_to_qpdf_page_composition() {
    let runtime_root = fixture_runtime_root();
    let source = runtime_root.path().join("source with spaces.pdf");
    let output = runtime_root.path().join("range.tmp");
    fs::write(&source, b"source").unwrap();
    fs::write(&output, b"").unwrap();
    let runner = Arc::new(RecordingMergeRunner {
        calls: Mutex::new(Vec::new()),
    });
    let engine = QpdfCliEngine::with_runner(
        QpdfRuntimeResolver::from_root(runtime_root.path()),
        runner.clone(),
    );

    engine
        .create_page_range(&StructuralPdfPageRangeRequest {
            source_path: source.clone(),
            first_page: 7,
            last_page: 12,
            working_output_path: output.clone(),
        })
        .unwrap();

    let calls = runner.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1][0], "--empty");
    assert_eq!(calls[1][1], "--pages");
    assert_eq!(
        PathBuf::from(&calls[1][2]),
        std::path::absolute(source).unwrap()
    );
    assert_eq!(calls[1][3], "7-12");
    assert_eq!(calls[1][4], "--");
    assert_eq!(
        PathBuf::from(&calls[1][5]),
        std::path::absolute(output).unwrap()
    );
}

#[test]
fn page_range_rejects_zero_and_descending_ranges_before_launch() {
    let runtime_root = fixture_runtime_root();
    let source = runtime_root.path().join("source.pdf");
    fs::write(&source, b"source").unwrap();
    let runner = Arc::new(RecordingMergeRunner {
        calls: Mutex::new(Vec::new()),
    });
    let engine = QpdfCliEngine::with_runner(
        QpdfRuntimeResolver::from_root(runtime_root.path()),
        runner.clone(),
    );

    for (first_page, last_page) in [(0, 1), (4, 3)] {
        assert_eq!(
            engine.create_page_range(&StructuralPdfPageRangeRequest {
                source_path: source.clone(),
                first_page,
                last_page,
                working_output_path: runtime_root.path().join("output.pdf"),
            }),
            Err(StructuralPdfError::OperationFailed)
        );
    }
    assert!(runner.calls.lock().unwrap().is_empty());
}

#[test]
fn rewrite_rejects_an_output_alias_of_the_source_before_launching_qpdf() {
    let runtime_root = fixture_runtime_root();
    let source = runtime_root.path().join("document.pdf");
    fs::write(&source, b"fixture").unwrap();
    let runner = FakeRunner {
        outputs: Mutex::new(VecDeque::new()),
    };
    let engine = QpdfCliEngine::with_runner(
        QpdfRuntimeResolver::from_root(runtime_root.path()),
        Arc::new(runner),
    );
    let aliased_source = runtime_root.path().join(".").join("document.pdf");

    assert_eq!(
        engine.rewrite(&source, &aliased_source),
        Err(StructuralPdfError::OutputWriteFailed)
    );
}

fn fixture_runtime_root() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let manifest = runtime_manifest().expect("manifest should parse");
    for file in manifest.runtime_files {
        let path = directory.path().join(file.relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"fixture").unwrap();
    }
    directory
}
