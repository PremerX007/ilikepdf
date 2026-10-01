use std::collections::VecDeque;
use std::sync::Mutex;

use crate::manifest::runtime_manifest;
use crate::process::BoundedDiagnostic;
use ilikepdf_core::StructuralPdfPagePlanItem;

use super::*;

struct FakeRunner {
    outputs: Mutex<VecDeque<Result<QpdfProcessOutput, QpdfProcessError>>>,
}

struct RecordingMergeRunner {
    calls: Mutex<Vec<Vec<OsString>>>,
}

type RecordedSecurityCall = (Vec<OsString>, Option<Vec<u8>>);

struct RecordingSecurityRunner {
    calls: Mutex<Vec<RecordedSecurityCall>>,
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

impl QpdfRunner for RecordingSecurityRunner {
    fn run(
        &self,
        _executable: &Path,
        arguments: &[OsString],
        stdin_data: Option<&[u8]>,
    ) -> Result<QpdfProcessOutput, QpdfProcessError> {
        self.calls
            .lock()
            .unwrap()
            .push((arguments.to_vec(), stdin_data.map(<[u8]>::to_vec)));
        if arguments == [OsString::from("--version")] {
            return Ok(QpdfProcessOutput {
                exit_code: Some(0),
                stdout: diagnostic(b"qpdf version 12.4.1\n"),
                stderr: diagnostic(b""),
            });
        }
        if arguments.first() == Some(&OsString::from("--requires-password")) {
            let source = PathBuf::from(arguments.last().unwrap());
            let name = source.file_name().unwrap().to_string_lossy();
            return Ok(QpdfProcessOutput {
                exit_code: Some(if name.contains("required") { 0 } else { 2 }),
                stdout: diagnostic(b""),
                stderr: diagnostic(b""),
            });
        }
        let sensitive = stdin_data.expect("security call should use sensitive stdin");
        let lines = String::from_utf8(sensitive.to_vec()).unwrap();
        let exit_code = if lines.contains("--password=wrong-test-value")
            && lines.contains("--requires-password")
        {
            0
        } else {
            0.max(if lines.contains("--requires-password") {
                3
            } else {
                0
            })
        };
        if !lines.contains("--requires-password") && !lines.contains("--check") {
            let output = PathBuf::from(lines.lines().last().unwrap());
            fs::write(output, b"secured output").unwrap();
        }
        Ok(QpdfProcessOutput {
            exit_code: Some(exit_code),
            stdout: diagnostic(b""),
            stderr: diagnostic(b""),
        })
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
    let source = repository_root
        .join("crates")
        .join("ilikepdf_pdf")
        .join("tests")
        .join("fixtures")
        .join("one_page.pdf");
    let directory = tempfile::tempdir().unwrap();
    let protected = directory.path().join("password protected.pdf");
    let engine = QpdfCliEngine::from_runtime_root(runtime_root);
    let password = SecretString::new("merge-test-password".to_owned());
    engine
        .protect(&StructuralPdfProtectRequest {
            source_path: source,
            working_output_path: protected.clone(),
            open_password: &password,
        })
        .expect("test fixture encryption should succeed through secure stdin");

    assert_eq!(
        engine.validate(&protected),
        Err(StructuralPdfError::PasswordRequired)
    );
    let failure = ilikepdf_core::split_pdf(
        &engine,
        ilikepdf_core::SplitPdfRequest {
            source_path: protected.clone(),
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
    let organize_error =
        ilikepdf_core::inspect_organize_pdf_sources(&engine, &[], std::slice::from_ref(&protected))
            .expect_err("password-protected input must not enter an organize session");
    assert_eq!(
        organize_error.code,
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
fn page_plan_maps_cross_source_order_and_relative_rotations_to_qpdf() {
    let runtime_root = fixture_runtime_root();
    let first = runtime_root.path().join("first source.pdf");
    let second = runtime_root.path().join("second.pdf");
    let output = runtime_root.path().join("organized.tmp");
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
        .create_page_plan(&StructuralPdfPagePlanRequest {
            ordered_pages: vec![
                StructuralPdfPagePlanItem {
                    source_path: second.clone(),
                    page_number: 2,
                    rotation: StructuralPdfPageRotation::Clockwise90,
                },
                StructuralPdfPagePlanItem {
                    source_path: first.clone(),
                    page_number: 1,
                    rotation: StructuralPdfPageRotation::None,
                },
                StructuralPdfPagePlanItem {
                    source_path: second.clone(),
                    page_number: 1,
                    rotation: StructuralPdfPageRotation::CounterClockwise90,
                },
                StructuralPdfPagePlanItem {
                    source_path: first.clone(),
                    page_number: 3,
                    rotation: StructuralPdfPageRotation::HalfTurn,
                },
            ],
            working_output_path: output.clone(),
        })
        .unwrap();

    let calls = runner.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1][0], "--empty");
    assert_eq!(calls[1][1], "--pages");
    assert_eq!(
        PathBuf::from(&calls[1][2]),
        std::path::absolute(second).unwrap()
    );
    assert_eq!(calls[1][3], "2");
    assert_eq!(
        PathBuf::from(&calls[1][4]),
        std::path::absolute(first).unwrap()
    );
    assert_eq!(calls[1][5], "1");
    assert_eq!(calls[1][6], calls[1][2]);
    assert_eq!(calls[1][7], "1");
    assert_eq!(calls[1][8], calls[1][4]);
    assert_eq!(calls[1][9], "3");
    assert_eq!(calls[1][10], "--");
    assert_eq!(calls[1][11], "--rotate=+90:1");
    assert_eq!(calls[1][12], "--rotate=-90:3");
    assert_eq!(calls[1][13], "--rotate=+180:4");
    assert_eq!(
        PathBuf::from(&calls[1][14]),
        std::path::absolute(output).unwrap()
    );
}

#[test]
fn page_plan_rejects_empty_or_zero_page_items_before_launch() {
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

    for ordered_pages in [
        Vec::new(),
        vec![StructuralPdfPagePlanItem {
            source_path: source.clone(),
            page_number: 0,
            rotation: StructuralPdfPageRotation::None,
        }],
    ] {
        assert_eq!(
            engine.create_page_plan(&StructuralPdfPagePlanRequest {
                ordered_pages,
                working_output_path: runtime_root.path().join("output.pdf"),
            }),
            Err(StructuralPdfError::OperationFailed)
        );
    }
    assert!(runner.calls.lock().unwrap().is_empty());
}

#[test]
fn protect_places_both_passwords_only_in_sensitive_stdin() {
    let runtime_root = fixture_runtime_root();
    let source = runtime_root.path().join("source.pdf");
    let output = runtime_root.path().join("protected.pdf");
    fs::write(&source, b"source").unwrap();
    let runner = Arc::new(RecordingSecurityRunner {
        calls: Mutex::new(Vec::new()),
    });
    let engine = QpdfCliEngine::with_runner(
        QpdfRuntimeResolver::from_root(runtime_root.path()),
        runner.clone(),
    );
    let password = SecretString::new("รหัส-test-value".to_owned());

    engine
        .protect(&StructuralPdfProtectRequest {
            source_path: source,
            working_output_path: output,
            open_password: &password,
        })
        .unwrap();

    let calls = runner.calls.lock().unwrap();
    assert!(calls.iter().all(|(arguments, _)| {
        !arguments.iter().any(|argument| {
            argument
                .to_string_lossy()
                .contains(password.expose_secret())
        })
    }));
    let (arguments, input) = calls.last().unwrap();
    assert_eq!(arguments, &[OsString::from("@-")]);
    let input = String::from_utf8(input.clone().unwrap()).unwrap();
    assert!(input.contains(&format!("--user-password={}", password.expose_secret())));
    let owner = input
        .lines()
        .find_map(|line| line.strip_prefix("--owner-password="))
        .unwrap();
    assert_eq!(owner.len(), 64);
    assert_ne!(owner, password.expose_secret());
    assert!(input.contains("--bits=256"));
    assert!(!input.contains("--allow-weak-crypto"));
}

#[test]
fn password_unlock_uses_at_stdin_and_wrong_password_is_typed() {
    let runtime_root = fixture_runtime_root();
    let source = runtime_root.path().join("password-required.pdf");
    fs::write(&source, b"source").unwrap();
    let runner = Arc::new(RecordingSecurityRunner {
        calls: Mutex::new(Vec::new()),
    });
    let engine = QpdfCliEngine::with_runner(
        QpdfRuntimeResolver::from_root(runtime_root.path()),
        runner.clone(),
    );
    let correct = SecretString::new("correct-test-value".to_owned());
    engine
        .unlock(&StructuralPdfUnlockRequest {
            source_path: source.clone(),
            working_output_path: runtime_root.path().join("unlocked.pdf"),
            password: Some(&correct),
        })
        .unwrap();

    let wrong = SecretString::new("wrong-test-value".to_owned());
    let error = engine
        .unlock(&StructuralPdfUnlockRequest {
            source_path: source,
            working_output_path: runtime_root.path().join("must-not-exist.pdf"),
            password: Some(&wrong),
        })
        .unwrap_err();
    assert_eq!(error, StructuralPdfError::IncorrectPassword);
    assert!(!format!("{error:?}").contains(wrong.expose_secret()));
    assert!(!runtime_root.path().join("must-not-exist.pdf").exists());

    let calls = runner.calls.lock().unwrap();
    assert!(calls.iter().all(|(arguments, _)| {
        !arguments.iter().any(|argument| {
            let argument = argument.to_string_lossy();
            argument.contains(correct.expose_secret()) || argument.contains(wrong.expose_secret())
        })
    }));
    assert!(
        calls
            .iter()
            .filter(|(arguments, input)| arguments == &[OsString::from("@-")] && input.is_some())
            .count()
            >= 3
    );
}

#[test]
fn secret_bearing_domain_requests_have_redacted_debug_output() {
    let secret = SecretString::new("request-secret-value".to_owned());
    let protect = StructuralPdfProtectRequest {
        source_path: PathBuf::from("private-source.pdf"),
        working_output_path: PathBuf::from("private-output.pdf"),
        open_password: &secret,
    };
    let unlock = StructuralPdfUnlockRequest {
        source_path: PathBuf::from("private-source.pdf"),
        working_output_path: PathBuf::from("private-output.pdf"),
        password: Some(&secret),
    };

    for debug in [format!("{protect:?}"), format!("{unlock:?}")] {
        assert!(!debug.contains(secret.expose_secret()));
        assert!(!debug.contains("private-source.pdf"));
        assert!(debug.contains("[REDACTED]"));
    }
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
