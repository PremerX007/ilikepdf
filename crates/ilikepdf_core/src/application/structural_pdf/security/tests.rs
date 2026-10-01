use std::sync::Mutex;

use super::*;
use crate::{
    StructuralPdfEngineFamily, StructuralPdfEngineInfo, StructuralPdfMergeRequest,
    StructuralPdfOperationResult, StructuralPdfPagePlanRequest, StructuralPdfPageRangeRequest,
    StructuralPdfValidation, StructuralPdfVersion,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FakeOperation {
    Protect,
    Unlock,
}

struct FakeSecurityEngine {
    source_path: PathBuf,
    source_state: PdfEncryptionState,
    operation: FakeOperation,
    reject_password: bool,
    calls: Mutex<Vec<&'static str>>,
}

impl FakeSecurityEngine {
    fn new(
        source_path: PathBuf,
        source_state: PdfEncryptionState,
        operation: FakeOperation,
    ) -> Self {
        Self {
            source_path,
            source_state,
            operation,
            reject_password: false,
            calls: Mutex::new(Vec::new()),
        }
    }

    fn with_wrong_password(mut self) -> Self {
        self.reject_password = true;
        self
    }
}

impl StructuralPdfEngine for FakeSecurityEngine {
    fn probe(&self) -> Result<StructuralPdfEngineInfo, StructuralPdfError> {
        Ok(StructuralPdfEngineInfo {
            family: StructuralPdfEngineFamily::Qpdf,
            version: StructuralPdfVersion::new(12, 4, 1),
        })
    }

    fn validate(&self, _source_path: &Path) -> Result<StructuralPdfValidation, StructuralPdfError> {
        Ok(StructuralPdfValidation {
            has_warnings: false,
        })
    }

    fn inspect_encryption(
        &self,
        source_path: &Path,
    ) -> Result<PdfEncryptionState, StructuralPdfError> {
        if source_path == self.source_path {
            Ok(self.source_state)
        } else {
            Ok(match self.operation {
                FakeOperation::Protect => PdfEncryptionState::EncryptedPasswordRequired,
                FakeOperation::Unlock => PdfEncryptionState::Unencrypted,
            })
        }
    }

    fn validate_with_password(
        &self,
        _source_path: &Path,
        _password: &SecretString,
    ) -> Result<StructuralPdfValidation, StructuralPdfError> {
        if self.reject_password {
            Err(StructuralPdfError::IncorrectPassword)
        } else {
            Ok(StructuralPdfValidation {
                has_warnings: false,
            })
        }
    }

    fn rewrite(
        &self,
        _source_path: &Path,
        _working_output_path: &Path,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        unreachable!()
    }

    fn merge(
        &self,
        _request: &StructuralPdfMergeRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        unreachable!()
    }

    fn create_page_range(
        &self,
        _request: &StructuralPdfPageRangeRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        unreachable!()
    }

    fn create_page_plan(
        &self,
        _request: &StructuralPdfPagePlanRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        unreachable!()
    }

    fn protect(
        &self,
        request: &StructuralPdfProtectRequest<'_>,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        assert_eq!(self.operation, FakeOperation::Protect);
        assert!(!request.open_password.expose_secret().is_empty());
        self.calls.lock().unwrap().push("protect");
        fs::copy(&request.source_path, &request.working_output_path).unwrap();
        Ok(StructuralPdfOperationResult {
            has_warnings: false,
        })
    }

    fn unlock(
        &self,
        request: &StructuralPdfUnlockRequest<'_>,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        assert_eq!(self.operation, FakeOperation::Unlock);
        self.calls
            .lock()
            .unwrap()
            .push(if request.password.is_some() {
                "unlock-with-password"
            } else {
                "unlock-without-password"
            });
        fs::copy(&request.source_path, &request.working_output_path).unwrap();
        Ok(StructuralPdfOperationResult {
            has_warnings: false,
        })
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("ilikepdf_pdf")
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn copy_fixture(directory: &Path, name: &str) -> PathBuf {
    let source = directory.join(name);
    fs::copy(fixture("two_page.pdf"), &source).unwrap();
    source
}

struct TestSecurityPdfVerifier;

impl SecurityPdfVerifier for TestSecurityPdfVerifier {
    fn inspect(
        &self,
        source_path: &Path,
        password: Option<&str>,
    ) -> Result<u32, ilikepdf_pdf::PdfError> {
        let renderer = crate::test_support::pdf_renderer();
        match password {
            Some(password) => renderer.inspect_document_with_password(source_path, password),
            None => renderer.inspect_document(source_path),
        }
        .map(|info| info.page_count)
    }
}

#[test]
fn inspection_exposes_typed_three_state_behavior() {
    for state in [
        PdfEncryptionState::Unencrypted,
        PdfEncryptionState::EncryptedNoPasswordRequired,
        PdfEncryptionState::EncryptedPasswordRequired,
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = copy_fixture(directory.path(), "source.pdf");
        let engine = FakeSecurityEngine::new(source.clone(), state, FakeOperation::Unlock);

        let info =
            inspect_unlock_pdf_source_with_verifier(&engine, &TestSecurityPdfVerifier, &source)
                .unwrap();

        assert_eq!(info.encryption_state, state);
        assert_eq!(
            info.page_count,
            (state != PdfEncryptionState::EncryptedPasswordRequired).then_some(2)
        );
    }
}

#[test]
fn naming_is_deterministic_unicode_safe_and_filename_only() {
    assert_eq!(
        default_protect_pdf_output_name(Path::new("เอกสาร.pdf")).unwrap(),
        "เอกสาร-protected.pdf"
    );
    assert_eq!(
        default_unlock_pdf_output_name(Path::new("report-protected.pdf")).unwrap(),
        "report-protected-unlocked.pdf"
    );
    assert_eq!(
        normalize_secure_pdf_output_name("report").unwrap(),
        "report.pdf"
    );
    for invalid in ["../report.pdf", r"C:\report.pdf", "CON.pdf", "bad. "] {
        assert!(normalize_secure_pdf_output_name(invalid).is_err());
    }
}

#[test]
fn protect_validates_passwords_and_redacts_debug_output() {
    let directory = tempfile::tempdir().unwrap();
    let source = copy_fixture(directory.path(), "source.pdf");
    let engine = FakeSecurityEngine::new(
        source.clone(),
        PdfEncryptionState::Unencrypted,
        FakeOperation::Protect,
    );
    for (password, confirmation) in [("", ""), ("one", "two"), ("bad\nline", "bad\nline")] {
        let request = ProtectPdfRequest {
            source_path: source.clone(),
            destination_directory: directory.path().to_path_buf(),
            output_name: "protected.pdf".to_owned(),
            password: SecretString::new(password.to_owned()),
            confirmation: SecretString::new(confirmation.to_owned()),
        };
        let formatted = format!("{request:?}");
        assert!(!formatted.contains(password) || password.is_empty());
        assert!(
            protect_pdf_with_verifier(&engine, &TestSecurityPdfVerifier, request, |_| {}).is_err()
        );
    }
    assert!(engine.calls.lock().unwrap().is_empty());
}

#[test]
fn protect_publishes_only_after_validation_with_collision_and_preserves_source() {
    let directory = tempfile::tempdir().unwrap();
    let source = copy_fixture(directory.path(), "เอกสาร.pdf");
    let before = fs::read(&source).unwrap();
    fs::write(directory.path().join("เอกสาร-protected.PDF"), b"occupied").unwrap();
    let engine = FakeSecurityEngine::new(
        source.clone(),
        PdfEncryptionState::Unencrypted,
        FakeOperation::Protect,
    );
    let mut stages = Vec::new();

    let result = protect_pdf_with_verifier(
        &engine,
        &TestSecurityPdfVerifier,
        ProtectPdfRequest {
            source_path: source.clone(),
            destination_directory: directory.path().to_path_buf(),
            output_name: "เอกสาร-protected.pdf".to_owned(),
            password: SecretString::new("รหัสผ่าน".to_owned()),
            confirmation: SecretString::new("รหัสผ่าน".to_owned()),
        },
        |progress| stages.push(progress.stage),
    )
    .unwrap();

    assert_eq!(
        result.output_path,
        directory.path().join("เอกสาร-protected (1).pdf")
    );
    assert_eq!(result.page_count, 2);
    assert_eq!(fs::read(source).unwrap(), before);
    assert_eq!(
        fs::read(directory.path().join("เอกสาร-protected.PDF")).unwrap(),
        b"occupied"
    );
    assert_eq!(
        stages,
        [
            ProtectPdfStage::Preparing,
            ProtectPdfStage::Protecting,
            ProtectPdfStage::Validating,
            ProtectPdfStage::Publishing,
            ProtectPdfStage::Completed,
        ]
    );
}

#[test]
fn protect_rejects_any_encrypted_source_without_publication() {
    for state in [
        PdfEncryptionState::EncryptedNoPasswordRequired,
        PdfEncryptionState::EncryptedPasswordRequired,
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = copy_fixture(directory.path(), "source.pdf");
        let engine = FakeSecurityEngine::new(source.clone(), state, FakeOperation::Protect);
        let error = protect_pdf_with_verifier(
            &engine,
            &TestSecurityPdfVerifier,
            ProtectPdfRequest {
                source_path: source,
                destination_directory: directory.path().to_path_buf(),
                output_name: "blocked.pdf".to_owned(),
                password: SecretString::new("valid".to_owned()),
                confirmation: SecretString::new("valid".to_owned()),
            },
            |_| {},
        )
        .unwrap_err();
        assert_eq!(error.error.code, ApplicationErrorCode::InvalidRequest);
        assert!(!directory.path().join("blocked.pdf").exists());
    }
}

#[test]
fn unlock_branches_between_unencrypted_no_password_and_required_password() {
    let directory = tempfile::tempdir().unwrap();
    let source = copy_fixture(directory.path(), "source.pdf");
    let unencrypted = FakeSecurityEngine::new(
        source.clone(),
        PdfEncryptionState::Unencrypted,
        FakeOperation::Unlock,
    );
    let error = unlock_pdf_with_verifier(
        &unencrypted,
        &TestSecurityPdfVerifier,
        UnlockPdfRequest {
            source_path: source.clone(),
            destination_directory: directory.path().to_path_buf(),
            output_name: "no-op.pdf".to_owned(),
            password: None,
        },
        |_| {},
    )
    .unwrap_err();
    assert_eq!(error.error.code, ApplicationErrorCode::InvalidRequest);

    let no_password = FakeSecurityEngine::new(
        source.clone(),
        PdfEncryptionState::EncryptedNoPasswordRequired,
        FakeOperation::Unlock,
    );
    let result = unlock_pdf_with_verifier(
        &no_password,
        &TestSecurityPdfVerifier,
        UnlockPdfRequest {
            source_path: source.clone(),
            destination_directory: directory.path().to_path_buf(),
            output_name: "direct.pdf".to_owned(),
            password: None,
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(result.page_count, 2);
    assert_eq!(
        no_password.calls.lock().unwrap().as_slice(),
        ["unlock-without-password"]
    );

    let required = FakeSecurityEngine::new(
        source.clone(),
        PdfEncryptionState::EncryptedPasswordRequired,
        FakeOperation::Unlock,
    );
    let missing = unlock_pdf_with_verifier(
        &required,
        &TestSecurityPdfVerifier,
        UnlockPdfRequest {
            source_path: source,
            destination_directory: directory.path().to_path_buf(),
            output_name: "missing.pdf".to_owned(),
            password: None,
        },
        |_| {},
    )
    .unwrap_err();
    assert_eq!(missing.error.code, ApplicationErrorCode::PasswordRequired);
}

#[test]
fn wrong_password_is_typed_retryable_and_publishes_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let source = copy_fixture(directory.path(), "source.pdf");
    let before = fs::read(&source).unwrap();
    let engine = FakeSecurityEngine::new(
        source.clone(),
        PdfEncryptionState::EncryptedPasswordRequired,
        FakeOperation::Unlock,
    )
    .with_wrong_password();

    let failure = unlock_pdf_with_verifier(
        &engine,
        &TestSecurityPdfVerifier,
        UnlockPdfRequest {
            source_path: source.clone(),
            destination_directory: directory.path().to_path_buf(),
            output_name: "wrong.pdf".to_owned(),
            password: Some(SecretString::new("incorrect-test-value".to_owned())),
        },
        |_| {},
    )
    .unwrap_err();

    assert_eq!(failure.error.code, ApplicationErrorCode::IncorrectPassword);
    assert!(!format!("{failure:?}").contains("incorrect-test-value"));
    assert!(!directory.path().join("wrong.pdf").exists());
    assert_eq!(fs::read(source).unwrap(), before);
}

#[test]
fn correct_password_unlocks_and_preserves_source() {
    let directory = tempfile::tempdir().unwrap();
    let source = copy_fixture(directory.path(), "report-protected.pdf");
    let before = fs::read(&source).unwrap();
    let engine = FakeSecurityEngine::new(
        source.clone(),
        PdfEncryptionState::EncryptedPasswordRequired,
        FakeOperation::Unlock,
    );

    let result = unlock_pdf_with_verifier(
        &engine,
        &TestSecurityPdfVerifier,
        UnlockPdfRequest {
            source_path: source.clone(),
            destination_directory: directory.path().to_path_buf(),
            output_name: default_unlock_pdf_output_name(&source).unwrap(),
            password: Some(SecretString::new("correct".to_owned())),
        },
        |_| {},
    )
    .unwrap();

    assert_eq!(
        result.output_path,
        directory.path().join("report-protected-unlocked.pdf")
    );
    assert_eq!(
        engine.calls.lock().unwrap().as_slice(),
        ["unlock-with-password"]
    );
    assert_eq!(fs::read(source).unwrap(), before);
}
