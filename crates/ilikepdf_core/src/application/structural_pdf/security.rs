use std::fmt::{self, Debug, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    PdfEncryptionState, StructuralPdfEngine, StructuralPdfError, StructuralPdfProtectRequest,
    StructuralPdfUnlockRequest,
};
use crate::application::output::PendingNumberedPdfOutput;
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult, SecretString};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectPdfSourceInfo {
    pub source_path: PathBuf,
    pub page_count: Option<u32>,
    pub size_bytes: u64,
    pub encryption_state: PdfEncryptionState,
    pub has_warnings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockPdfSourceInfo {
    pub source_path: PathBuf,
    pub page_count: Option<u32>,
    pub size_bytes: u64,
    pub encryption_state: PdfEncryptionState,
    pub has_warnings: bool,
}

pub struct ProtectPdfRequest {
    pub source_path: PathBuf,
    pub destination_directory: PathBuf,
    pub output_name: String,
    pub password: SecretString,
    pub confirmation: SecretString,
}

impl Debug for ProtectPdfRequest {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectPdfRequest")
            .field("source_path", &"[PATH]")
            .field("destination_directory", &"[PATH]")
            .field("output_name", &self.output_name)
            .field("password", &"[REDACTED]")
            .field("confirmation", &"[REDACTED]")
            .finish()
    }
}

pub struct UnlockPdfRequest {
    pub source_path: PathBuf,
    pub destination_directory: PathBuf,
    pub output_name: String,
    pub password: Option<SecretString>,
}

impl Debug for UnlockPdfRequest {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnlockPdfRequest")
            .field("source_path", &"[PATH]")
            .field("destination_directory", &"[PATH]")
            .field("output_name", &self.output_name)
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectPdfStage {
    Preparing,
    Protecting,
    Validating,
    Publishing,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectPdfProgress {
    pub stage: ProtectPdfStage,
    pub page_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectPdfResult {
    pub output_path: PathBuf,
    pub page_count: u32,
    pub has_warnings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectPdfFailure {
    pub error: ApplicationError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlockPdfStage {
    Preparing,
    Unlocking,
    Validating,
    Publishing,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnlockPdfProgress {
    pub stage: UnlockPdfStage,
    pub page_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockPdfResult {
    pub output_path: PathBuf,
    pub page_count: u32,
    pub has_warnings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockPdfFailure {
    pub error: ApplicationError,
}

pub fn inspect_protect_pdf_source(
    engine: &dyn StructuralPdfEngine,
    source_path: &Path,
) -> ApplicationResult<ProtectPdfSourceInfo> {
    inspect_protect_pdf_source_with_verifier(engine, &NativeSecurityPdfVerifier, source_path)
}

fn inspect_protect_pdf_source_with_verifier(
    engine: &dyn StructuralPdfEngine,
    verifier: &dyn SecurityPdfVerifier,
    source_path: &Path,
) -> ApplicationResult<ProtectPdfSourceInfo> {
    let source = inspect_source(engine, verifier, source_path)?;
    Ok(ProtectPdfSourceInfo {
        source_path: source.source_path,
        page_count: source.page_count,
        size_bytes: source.size_bytes,
        encryption_state: source.encryption_state,
        has_warnings: source.has_warnings,
    })
}

pub fn inspect_unlock_pdf_source(
    engine: &dyn StructuralPdfEngine,
    source_path: &Path,
) -> ApplicationResult<UnlockPdfSourceInfo> {
    inspect_unlock_pdf_source_with_verifier(engine, &NativeSecurityPdfVerifier, source_path)
}

fn inspect_unlock_pdf_source_with_verifier(
    engine: &dyn StructuralPdfEngine,
    verifier: &dyn SecurityPdfVerifier,
    source_path: &Path,
) -> ApplicationResult<UnlockPdfSourceInfo> {
    let source = inspect_source(engine, verifier, source_path)?;
    Ok(UnlockPdfSourceInfo {
        source_path: source.source_path,
        page_count: source.page_count,
        size_bytes: source.size_bytes,
        encryption_state: source.encryption_state,
        has_warnings: source.has_warnings,
    })
}

pub fn default_protect_pdf_output_name(source_path: &Path) -> ApplicationResult<String> {
    default_output_name(source_path, "protected")
}

pub fn default_unlock_pdf_output_name(source_path: &Path) -> ApplicationResult<String> {
    default_output_name(source_path, "unlocked")
}

pub fn normalize_secure_pdf_output_name(output_name: &str) -> ApplicationResult<String> {
    crate::application::output::normalize_pdf_filename(output_name)
        .map(|name| name.to_string_lossy().into_owned())
}

pub fn protect_pdf(
    engine: &dyn StructuralPdfEngine,
    request: ProtectPdfRequest,
    on_progress: impl FnMut(ProtectPdfProgress),
) -> Result<ProtectPdfResult, ProtectPdfFailure> {
    protect_pdf_with_verifier(engine, &NativeSecurityPdfVerifier, request, on_progress)
}

fn protect_pdf_with_verifier(
    engine: &dyn StructuralPdfEngine,
    verifier: &dyn SecurityPdfVerifier,
    request: ProtectPdfRequest,
    mut on_progress: impl FnMut(ProtectPdfProgress),
) -> Result<ProtectPdfResult, ProtectPdfFailure> {
    validate_new_password(&request.password, &request.confirmation)
        .map_err(|error| ProtectPdfFailure { error })?;
    let pending = PendingNumberedPdfOutput::in_directory(
        &request.destination_directory,
        &request.output_name,
    )
    .map_err(|error| ProtectPdfFailure { error })?;

    on_progress(ProtectPdfProgress {
        stage: ProtectPdfStage::Preparing,
        page_count: 0,
    });
    validate_source_path(&request.source_path).map_err(|error| ProtectPdfFailure { error })?;
    match engine
        .inspect_encryption(&request.source_path)
        .map_err(|error| ProtectPdfFailure {
            error: map_engine_error(error),
        })? {
        PdfEncryptionState::Unencrypted => {}
        PdfEncryptionState::EncryptedNoPasswordRequired
        | PdfEncryptionState::EncryptedPasswordRequired => {
            return Err(ProtectPdfFailure {
                error: ApplicationError::new(
                    ApplicationErrorCode::InvalidRequest,
                    "This PDF is already encrypted. Unlock it before applying a new password",
                ),
            });
        }
    }
    let source_validation =
        engine
            .validate(&request.source_path)
            .map_err(|error| ProtectPdfFailure {
                error: map_engine_error(error),
            })?;
    let source_page_count = verifier
        .inspect(&request.source_path, None)
        .map_err(ApplicationError::from)
        .map_err(|error| ProtectPdfFailure { error })?;

    on_progress(ProtectPdfProgress {
        stage: ProtectPdfStage::Protecting,
        page_count: source_page_count,
    });
    let operation = engine
        .protect(&StructuralPdfProtectRequest {
            source_path: request.source_path.clone(),
            working_output_path: pending.working_path().to_path_buf(),
            open_password: &request.password,
        })
        .map_err(|error| ProtectPdfFailure {
            error: map_engine_error(error),
        })?;

    on_progress(ProtectPdfProgress {
        stage: ProtectPdfStage::Validating,
        page_count: source_page_count,
    });
    validate_working_file(pending.working_path()).map_err(|error| ProtectPdfFailure { error })?;
    if engine
        .inspect_encryption(pending.working_path())
        .map_err(|_| ProtectPdfFailure {
            error: output_validation_error("The protected PDF could not be verified"),
        })?
        != PdfEncryptionState::EncryptedPasswordRequired
    {
        return Err(ProtectPdfFailure {
            error: output_validation_error("The protected PDF does not require a password"),
        });
    }
    engine
        .validate_with_password(pending.working_path(), &request.password)
        .map_err(|_| ProtectPdfFailure {
            error: output_validation_error("The protected PDF could not be opened securely"),
        })?;
    let output_page_count = verifier
        .inspect(
            pending.working_path(),
            Some(request.password.expose_secret()),
        )
        .map_err(|_| ProtectPdfFailure {
            error: output_validation_error("PDFium could not open the protected PDF"),
        })?;
    if output_page_count != source_page_count {
        return Err(ProtectPdfFailure {
            error: output_validation_error("The protected PDF page count changed unexpectedly"),
        });
    }

    on_progress(ProtectPdfProgress {
        stage: ProtectPdfStage::Publishing,
        page_count: source_page_count,
    });
    let output_path = pending
        .publish()
        .map_err(|error| ProtectPdfFailure { error })?;
    on_progress(ProtectPdfProgress {
        stage: ProtectPdfStage::Completed,
        page_count: source_page_count,
    });
    Ok(ProtectPdfResult {
        output_path,
        page_count: source_page_count,
        has_warnings: source_validation.has_warnings || operation.has_warnings,
    })
}

pub fn unlock_pdf(
    engine: &dyn StructuralPdfEngine,
    request: UnlockPdfRequest,
    on_progress: impl FnMut(UnlockPdfProgress),
) -> Result<UnlockPdfResult, UnlockPdfFailure> {
    unlock_pdf_with_verifier(engine, &NativeSecurityPdfVerifier, request, on_progress)
}

fn unlock_pdf_with_verifier(
    engine: &dyn StructuralPdfEngine,
    verifier: &dyn SecurityPdfVerifier,
    request: UnlockPdfRequest,
    mut on_progress: impl FnMut(UnlockPdfProgress),
) -> Result<UnlockPdfResult, UnlockPdfFailure> {
    if let Some(password) = request.password.as_ref() {
        validate_transport_password(password).map_err(|error| UnlockPdfFailure { error })?;
    }
    let pending = PendingNumberedPdfOutput::in_directory(
        &request.destination_directory,
        &request.output_name,
    )
    .map_err(|error| UnlockPdfFailure { error })?;
    on_progress(UnlockPdfProgress {
        stage: UnlockPdfStage::Preparing,
        page_count: 0,
    });
    validate_source_path(&request.source_path).map_err(|error| UnlockPdfFailure { error })?;
    let state = engine
        .inspect_encryption(&request.source_path)
        .map_err(|error| UnlockPdfFailure {
            error: map_engine_error(error),
        })?;
    let password = match state {
        PdfEncryptionState::Unencrypted => {
            return Err(UnlockPdfFailure {
                error: ApplicationError::new(
                    ApplicationErrorCode::InvalidRequest,
                    "This PDF is already unlocked",
                ),
            });
        }
        PdfEncryptionState::EncryptedNoPasswordRequired => None,
        PdfEncryptionState::EncryptedPasswordRequired => {
            let password = request.password.as_ref().ok_or_else(|| UnlockPdfFailure {
                error: ApplicationError::new(
                    ApplicationErrorCode::PasswordRequired,
                    "Enter the password required to open this PDF",
                ),
            })?;
            if password.expose_secret().is_empty() {
                return Err(UnlockPdfFailure {
                    error: ApplicationError::new(
                        ApplicationErrorCode::PasswordRequired,
                        "Enter the password required to open this PDF",
                    ),
                });
            }
            Some(password)
        }
    };
    let source_validation = match password {
        Some(password) => engine.validate_with_password(&request.source_path, password),
        None => engine.validate(&request.source_path),
    }
    .map_err(|error| UnlockPdfFailure {
        error: map_engine_error(error),
    })?;
    let source_page_count = verifier
        .inspect(
            &request.source_path,
            password.map(SecretString::expose_secret),
        )
        .map_err(|error| UnlockPdfFailure {
            error: match error.kind {
                ilikepdf_pdf::PdfErrorKind::PasswordRequired => incorrect_password_error(),
                _ => ApplicationError::from(error),
            },
        })?;

    on_progress(UnlockPdfProgress {
        stage: UnlockPdfStage::Unlocking,
        page_count: source_page_count,
    });
    let operation = engine
        .unlock(&StructuralPdfUnlockRequest {
            source_path: request.source_path.clone(),
            working_output_path: pending.working_path().to_path_buf(),
            password,
        })
        .map_err(|error| UnlockPdfFailure {
            error: map_engine_error(error),
        })?;

    on_progress(UnlockPdfProgress {
        stage: UnlockPdfStage::Validating,
        page_count: source_page_count,
    });
    validate_working_file(pending.working_path()).map_err(|error| UnlockPdfFailure { error })?;
    if engine
        .inspect_encryption(pending.working_path())
        .map_err(|_| UnlockPdfFailure {
            error: output_validation_error("The unlocked PDF could not be verified"),
        })?
        != PdfEncryptionState::Unencrypted
    {
        return Err(UnlockPdfFailure {
            error: output_validation_error("The output PDF is still encrypted"),
        });
    }
    engine
        .validate(pending.working_path())
        .map_err(|_| UnlockPdfFailure {
            error: output_validation_error("The unlocked PDF failed structural validation"),
        })?;
    let output_page_count = verifier
        .inspect(pending.working_path(), None)
        .map_err(|_| UnlockPdfFailure {
            error: output_validation_error("PDFium could not open the unlocked PDF"),
        })?;
    if output_page_count != source_page_count {
        return Err(UnlockPdfFailure {
            error: output_validation_error("The unlocked PDF page count changed unexpectedly"),
        });
    }

    on_progress(UnlockPdfProgress {
        stage: UnlockPdfStage::Publishing,
        page_count: source_page_count,
    });
    let output_path = pending
        .publish()
        .map_err(|error| UnlockPdfFailure { error })?;
    on_progress(UnlockPdfProgress {
        stage: UnlockPdfStage::Completed,
        page_count: source_page_count,
    });
    Ok(UnlockPdfResult {
        output_path,
        page_count: source_page_count,
        has_warnings: source_validation.has_warnings || operation.has_warnings,
    })
}

struct InspectedSource {
    source_path: PathBuf,
    page_count: Option<u32>,
    size_bytes: u64,
    encryption_state: PdfEncryptionState,
    has_warnings: bool,
}

fn inspect_source(
    engine: &dyn StructuralPdfEngine,
    verifier: &dyn SecurityPdfVerifier,
    source_path: &Path,
) -> ApplicationResult<InspectedSource> {
    let metadata = validate_source_path(source_path)?;
    let source_path = std::path::absolute(source_path).map_err(|_| {
        ApplicationError::new(
            ApplicationErrorCode::SourceUnreadable,
            "The selected PDF could not be read",
        )
    })?;
    let encryption_state = engine
        .inspect_encryption(&source_path)
        .map_err(map_engine_error)?;
    let (page_count, has_warnings) = match encryption_state {
        PdfEncryptionState::EncryptedPasswordRequired => (None, false),
        PdfEncryptionState::Unencrypted | PdfEncryptionState::EncryptedNoPasswordRequired => {
            let validation = engine.validate(&source_path).map_err(map_engine_error)?;
            let page_count = verifier.inspect(&source_path, None)?;
            (Some(page_count), validation.has_warnings)
        }
    };
    Ok(InspectedSource {
        source_path,
        page_count,
        size_bytes: metadata.len(),
        encryption_state,
        has_warnings,
    })
}

trait SecurityPdfVerifier {
    fn inspect(
        &self,
        source_path: &Path,
        password: Option<&str>,
    ) -> Result<u32, ilikepdf_pdf::PdfError>;
}

struct NativeSecurityPdfVerifier;

impl SecurityPdfVerifier for NativeSecurityPdfVerifier {
    fn inspect(
        &self,
        source_path: &Path,
        password: Option<&str>,
    ) -> Result<u32, ilikepdf_pdf::PdfError> {
        match password {
            Some(password) => ilikepdf_pdf::inspect_document_with_password(source_path, password),
            None => ilikepdf_pdf::inspect_document(source_path),
        }
        .map(|info| info.page_count)
    }
}

fn default_output_name(source_path: &Path, suffix: &str) -> ApplicationResult<String> {
    let stem = source_path
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| {
            ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "The selected PDF does not have a usable file name",
            )
        })?;
    normalize_secure_pdf_output_name(&format!("{}-{suffix}.pdf", stem.to_string_lossy()))
}

fn validate_new_password(
    password: &SecretString,
    confirmation: &SecretString,
) -> ApplicationResult<()> {
    validate_transport_password(password)?;
    if password.expose_secret().is_empty() {
        return Err(ApplicationError::new(
            ApplicationErrorCode::InvalidRequest,
            "Enter a password to protect this PDF",
        ));
    }
    if password.expose_secret() != confirmation.expose_secret() {
        return Err(ApplicationError::new(
            ApplicationErrorCode::InvalidRequest,
            "The password confirmation does not match",
        ));
    }
    Ok(())
}

fn validate_transport_password(password: &SecretString) -> ApplicationResult<()> {
    if password
        .expose_secret()
        .chars()
        .any(|character| matches!(character, '\0' | '\r' | '\n'))
    {
        return Err(ApplicationError::new(
            ApplicationErrorCode::InvalidRequest,
            "Passwords cannot contain line breaks or NUL characters",
        ));
    }
    Ok(())
}

fn validate_source_path(source_path: &Path) -> ApplicationResult<fs::Metadata> {
    match fs::metadata(source_path) {
        Ok(metadata) if metadata.is_file() => Ok(metadata),
        Ok(_) => Err(ApplicationError::new(
            ApplicationErrorCode::SourceNotFile,
            "The selected path is not a file",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(ApplicationError::new(
            ApplicationErrorCode::SourceNotFound,
            "The selected PDF no longer exists",
        )),
        Err(_) => Err(ApplicationError::new(
            ApplicationErrorCode::SourceUnreadable,
            "The selected PDF could not be read",
        )),
    }
}

fn validate_working_file(path: &Path) -> ApplicationResult<()> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.len() > 0 => Ok(()),
        _ => Err(output_validation_error(
            "The private output PDF was not created correctly",
        )),
    }
}

fn incorrect_password_error() -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::IncorrectPassword,
        "The password is incorrect. Try again",
    )
}

fn output_validation_error(message: &'static str) -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::StructuralPdfOutputValidationFailed,
        message,
    )
}

fn map_engine_error(error: StructuralPdfError) -> ApplicationError {
    match error {
        StructuralPdfError::RuntimeUnavailable => ApplicationError::new(
            ApplicationErrorCode::StructuralPdfRuntimeUnavailable,
            "The local structural PDF runtime is unavailable",
        ),
        StructuralPdfError::RuntimeIncompatible => ApplicationError::new(
            ApplicationErrorCode::StructuralPdfRuntimeIncompatible,
            "The bundled structural PDF runtime is incompatible",
        ),
        StructuralPdfError::RuntimeLaunchFailed => ApplicationError::new(
            ApplicationErrorCode::StructuralPdfLaunchFailed,
            "The local structural PDF runtime could not be started",
        ),
        StructuralPdfError::SourceNotFound => ApplicationError::new(
            ApplicationErrorCode::SourceNotFound,
            "The selected PDF no longer exists",
        ),
        StructuralPdfError::SourceNotFile => ApplicationError::new(
            ApplicationErrorCode::SourceNotFile,
            "The selected path is not a file",
        ),
        StructuralPdfError::SourceUnreadable => ApplicationError::new(
            ApplicationErrorCode::SourceUnreadable,
            "The selected PDF could not be read",
        ),
        StructuralPdfError::PasswordRequired => ApplicationError::new(
            ApplicationErrorCode::PasswordRequired,
            "A password is required to open this PDF",
        ),
        StructuralPdfError::IncorrectPassword => incorrect_password_error(),
        StructuralPdfError::InvalidDocument => ApplicationError::new(
            ApplicationErrorCode::InvalidPdf,
            "The selected file is not a valid readable PDF",
        ),
        StructuralPdfError::OutputWriteFailed => ApplicationError::new(
            ApplicationErrorCode::OutputWriteFailed,
            "The output PDF could not be written",
        ),
        StructuralPdfError::OperationFailed => ApplicationError::new(
            ApplicationErrorCode::StructuralPdfOperationFailed,
            "The structural PDF operation failed",
        ),
    }
}

#[cfg(test)]
mod tests;
