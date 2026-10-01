//! Engine-neutral structural PDF capability contracts and application workflows.

use std::path::{Path, PathBuf};

mod organize;
mod security;
mod split;
mod workflow;

pub use organize::{
    OrganizePdfFailure, OrganizePdfPageItem, OrganizePdfPageRotation, OrganizePdfProgress,
    OrganizePdfRequest, OrganizePdfResult, OrganizePdfSession, OrganizePdfSource,
    OrganizePdfSourceInfo, OrganizePdfStage, PageRotationDirection, inspect_organize_pdf_sources,
    normalize_organize_pdf_output_name, organize_pdf,
};
pub use security::{
    ProtectPdfFailure, ProtectPdfProgress, ProtectPdfRequest, ProtectPdfResult,
    ProtectPdfSourceInfo, ProtectPdfStage, UnlockPdfFailure, UnlockPdfProgress, UnlockPdfRequest,
    UnlockPdfResult, UnlockPdfSourceInfo, UnlockPdfStage, default_protect_pdf_output_name,
    default_unlock_pdf_output_name, inspect_protect_pdf_source, inspect_unlock_pdf_source,
    normalize_secure_pdf_output_name, protect_pdf, unlock_pdf,
};
pub use split::{
    SplitPdfFailure, SplitPdfMode, SplitPdfPageRange, SplitPdfPart, SplitPdfProgress,
    SplitPdfRequest, SplitPdfResult, SplitPdfSourceInfo, SplitPdfStage, inspect_split_pdf_source,
    split_pdf,
};
pub use workflow::{
    merge_pdf, probe_structural_pdf_engine, rewrite_structural_pdf, validate_structural_pdf,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StructuralPdfEngineFamily {
    Qpdf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralPdfVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl StructuralPdfVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralPdfEngineInfo {
    pub family: StructuralPdfEngineFamily,
    pub version: StructuralPdfVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralPdfValidation {
    pub has_warnings: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralPdfOperationResult {
    pub has_warnings: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StructuralPdfError {
    RuntimeUnavailable,
    RuntimeIncompatible,
    RuntimeLaunchFailed,
    SourceNotFound,
    SourceNotFile,
    SourceUnreadable,
    PasswordRequired,
    IncorrectPassword,
    InvalidDocument,
    OutputWriteFailed,
    OperationFailed,
}

/// Structural capabilities consumed by application workflows.
///
/// Implementations may use a child process, an in-process library, or another
/// platform-appropriate mechanism. Process concepts are intentionally absent.
pub trait StructuralPdfEngine: Send + Sync {
    fn probe(&self) -> Result<StructuralPdfEngineInfo, StructuralPdfError>;

    fn validate(&self, source_path: &Path) -> Result<StructuralPdfValidation, StructuralPdfError>;

    /// Reports encryption access requirements without exposing implementation diagnostics.
    fn inspect_encryption(
        &self,
        _source_path: &Path,
    ) -> Result<PdfEncryptionState, StructuralPdfError> {
        Err(StructuralPdfError::OperationFailed)
    }

    /// Validates an encrypted document using a short-lived password secret.
    fn validate_with_password(
        &self,
        _source_path: &Path,
        _password: &crate::SecretString,
    ) -> Result<StructuralPdfValidation, StructuralPdfError> {
        Err(StructuralPdfError::OperationFailed)
    }

    /// Writes a content-preserving structural rewrite to `working_output_path`.
    /// The caller owns publication and must never pass the source path as output.
    fn rewrite(
        &self,
        source_path: &Path,
        working_output_path: &Path,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError>;

    /// Structurally copies every page from each ordered source into one private
    /// working output. The caller owns validation and publication.
    fn merge(
        &self,
        request: &StructuralPdfMergeRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError>;

    /// Structurally copies one inclusive, contiguous, one-based page range into
    /// a private working output. The caller owns policy, validation, and publication.
    fn create_page_range(
        &self,
        request: &StructuralPdfPageRangeRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError>;

    /// Builds one private PDF from an arbitrary ordered page plan. Each page
    /// keeps its source geometry and may receive a relative quarter-turn.
    fn create_page_plan(
        &self,
        _request: &StructuralPdfPagePlanRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        Err(StructuralPdfError::OperationFailed)
    }

    /// Creates an AES-256 encrypted private output. Permission and owner-secret
    /// details remain implementation concerns.
    fn protect(
        &self,
        _request: &StructuralPdfProtectRequest<'_>,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        Err(StructuralPdfError::OperationFailed)
    }

    /// Creates an unencrypted private output using the supplied password only
    /// when the source requires one.
    fn unlock(
        &self,
        _request: &StructuralPdfUnlockRequest<'_>,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        Err(StructuralPdfError::OperationFailed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfEncryptionState {
    Unencrypted,
    EncryptedNoPasswordRequired,
    EncryptedPasswordRequired,
}

pub struct StructuralPdfProtectRequest<'a> {
    pub source_path: PathBuf,
    pub working_output_path: PathBuf,
    pub open_password: &'a crate::SecretString,
}

impl std::fmt::Debug for StructuralPdfProtectRequest<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StructuralPdfProtectRequest")
            .field("source_path", &"[PATH]")
            .field("working_output_path", &"[PATH]")
            .field("open_password", &"[REDACTED]")
            .finish()
    }
}

pub struct StructuralPdfUnlockRequest<'a> {
    pub source_path: PathBuf,
    pub working_output_path: PathBuf,
    pub password: Option<&'a crate::SecretString>,
}

impl std::fmt::Debug for StructuralPdfUnlockRequest<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StructuralPdfUnlockRequest")
            .field("source_path", &"[PATH]")
            .field("working_output_path", &"[PATH]")
            .field("password", &self.password.map(|_| "[REDACTED]"))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralPdfMergeRequest {
    pub ordered_source_paths: Vec<PathBuf>,
    pub working_output_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralPdfPageRangeRequest {
    pub source_path: PathBuf,
    pub first_page: u32,
    pub last_page: u32,
    pub working_output_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralPdfPageRotation {
    None,
    Clockwise90,
    HalfTurn,
    CounterClockwise90,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralPdfPagePlanItem {
    pub source_path: PathBuf,
    pub page_number: u32,
    pub rotation: StructuralPdfPageRotation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralPdfPagePlanRequest {
    pub ordered_pages: Vec<StructuralPdfPagePlanItem>,
    pub working_output_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteStructuralPdfRequest {
    pub source_path: PathBuf,
    pub destination_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralPdfRewriteResult {
    pub output_path: PathBuf,
    pub page_count: u32,
    pub has_warnings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePdfRequest {
    pub source_paths: Vec<PathBuf>,
    pub destination_directory: PathBuf,
    pub output_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergePdfStage {
    Preparing,
    Merging,
    Validating,
    Publishing,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergePdfProgress {
    pub stage: MergePdfStage,
    pub input_count: u32,
    pub total_page_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePdfResult {
    pub output_path: PathBuf,
    pub input_count: u32,
    pub page_count: u32,
    pub warning_input_count: u32,
    pub has_warnings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePdfFailure {
    pub error: crate::ApplicationError,
    pub input_index: Option<u32>,
    pub input_path: Option<PathBuf>,
    pub input_count: u32,
    pub expected_page_count: u32,
}
