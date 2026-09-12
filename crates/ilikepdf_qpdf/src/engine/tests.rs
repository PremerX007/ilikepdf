use std::collections::VecDeque;
use std::sync::Mutex;

use crate::manifest::runtime_manifest;
use crate::process::BoundedDiagnostic;

use super::*;

struct FakeRunner {
    outputs: Mutex<VecDeque<Result<QpdfProcessOutput, QpdfProcessError>>>,
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
