//! Engine-neutral structural PDF capability contracts and application workflows.

use std::path::{Path, PathBuf};

mod workflow;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralPdfMergeRequest {
    pub ordered_source_paths: Vec<PathBuf>,
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
