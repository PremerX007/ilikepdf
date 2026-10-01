use std::path::PathBuf;

use crate::frb_generated::StreamSink;

use super::application::ApplicationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfEncryptionState {
    Unencrypted,
    EncryptedNoPasswordRequired,
    EncryptedPasswordRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectPdfSourceInfo {
    pub source_path: String,
    pub page_count: Option<u32>,
    pub size_bytes: u64,
    pub encryption_state: PdfEncryptionState,
    pub has_warnings: bool,
    pub default_output_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockPdfSourceInfo {
    pub source_path: String,
    pub page_count: Option<u32>,
    pub size_bytes: u64,
    pub encryption_state: PdfEncryptionState,
    pub has_warnings: bool,
    pub default_output_name: String,
}

pub struct ProtectPdfRequest {
    pub source_path: String,
    pub destination_directory: String,
    pub output_name: String,
    pub password: String,
    pub confirmation: String,
}

pub struct UnlockPdfRequest {
    pub source_path: String,
    pub destination_directory: String,
    pub output_name: String,
    pub password: Option<String>,
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
pub enum UnlockPdfStage {
    Preparing,
    Unlocking,
    Validating,
    Publishing,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfSecurityStatus {
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectPdfUpdate {
    pub status: PdfSecurityStatus,
    pub stage: ProtectPdfStage,
    pub page_count: u32,
    pub output_path: Option<String>,
    pub has_warnings: bool,
    pub error: Option<ApplicationError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockPdfUpdate {
    pub status: PdfSecurityStatus,
    pub stage: UnlockPdfStage,
    pub page_count: u32,
    pub output_path: Option<String>,
    pub has_warnings: bool,
    pub error: Option<ApplicationError>,
}

pub fn inspect_protect_pdf_source(
    source_path: String,
) -> Result<ProtectPdfSourceInfo, ApplicationError> {
    crate::logging::record(crate::logging::Event::ProtectPdfInspectionRequested);
    let source_path = PathBuf::from(source_path);
    let engine = ilikepdf_qpdf::QpdfCliEngine::bundled();
    let info = ilikepdf_core::inspect_protect_pdf_source(&engine, &source_path)?;
    let default_output_name = ilikepdf_core::default_protect_pdf_output_name(&source_path)?;
    Ok(ProtectPdfSourceInfo {
        source_path: info.source_path.to_string_lossy().into_owned(),
        page_count: info.page_count,
        size_bytes: info.size_bytes,
        encryption_state: info.encryption_state.into(),
        has_warnings: info.has_warnings,
        default_output_name,
    })
}

pub fn inspect_unlock_pdf_source(
    source_path: String,
) -> Result<UnlockPdfSourceInfo, ApplicationError> {
    crate::logging::record(crate::logging::Event::UnlockPdfInspectionRequested);
    let source_path = PathBuf::from(source_path);
    let engine = ilikepdf_qpdf::QpdfCliEngine::bundled();
    let info = ilikepdf_core::inspect_unlock_pdf_source(&engine, &source_path)?;
    let default_output_name = ilikepdf_core::default_unlock_pdf_output_name(&source_path)?;
    Ok(UnlockPdfSourceInfo {
        source_path: info.source_path.to_string_lossy().into_owned(),
        page_count: info.page_count,
        size_bytes: info.size_bytes,
        encryption_state: info.encryption_state.into(),
        has_warnings: info.has_warnings,
        default_output_name,
    })
}

pub fn protect_pdf(request: ProtectPdfRequest, progress_sink: StreamSink<ProtectPdfUpdate>) {
    crate::logging::record(crate::logging::Event::ProtectPdfRequested);
    let engine = ilikepdf_qpdf::QpdfCliEngine::bundled();
    let mut last_stage = ProtectPdfStage::Preparing;
    let result = ilikepdf_core::protect_pdf(
        &engine,
        ilikepdf_core::ProtectPdfRequest {
            source_path: PathBuf::from(request.source_path),
            destination_directory: PathBuf::from(request.destination_directory),
            output_name: request.output_name,
            password: ilikepdf_core::SecretString::new(request.password),
            confirmation: ilikepdf_core::SecretString::new(request.confirmation),
        },
        |progress| {
            last_stage = progress.stage.into();
            let _ = progress_sink.add(ProtectPdfUpdate {
                status: PdfSecurityStatus::Running,
                stage: last_stage,
                page_count: progress.page_count,
                output_path: None,
                has_warnings: false,
                error: None,
            });
        },
    );
    let update = match result {
        Ok(result) => ProtectPdfUpdate {
            status: PdfSecurityStatus::Complete,
            stage: ProtectPdfStage::Completed,
            page_count: result.page_count,
            output_path: Some(result.output_path.to_string_lossy().into_owned()),
            has_warnings: result.has_warnings,
            error: None,
        },
        Err(failure) => ProtectPdfUpdate {
            status: PdfSecurityStatus::Failed,
            stage: last_stage,
            page_count: 0,
            output_path: None,
            has_warnings: false,
            error: Some(failure.error.into()),
        },
    };
    let _ = progress_sink.add(update);
}

pub fn unlock_pdf(request: UnlockPdfRequest, progress_sink: StreamSink<UnlockPdfUpdate>) {
    crate::logging::record(crate::logging::Event::UnlockPdfRequested);
    let engine = ilikepdf_qpdf::QpdfCliEngine::bundled();
    let mut last_stage = UnlockPdfStage::Preparing;
    let result = ilikepdf_core::unlock_pdf(
        &engine,
        ilikepdf_core::UnlockPdfRequest {
            source_path: PathBuf::from(request.source_path),
            destination_directory: PathBuf::from(request.destination_directory),
            output_name: request.output_name,
            password: request.password.map(ilikepdf_core::SecretString::new),
        },
        |progress| {
            last_stage = progress.stage.into();
            let _ = progress_sink.add(UnlockPdfUpdate {
                status: PdfSecurityStatus::Running,
                stage: last_stage,
                page_count: progress.page_count,
                output_path: None,
                has_warnings: false,
                error: None,
            });
        },
    );
    let update = match result {
        Ok(result) => UnlockPdfUpdate {
            status: PdfSecurityStatus::Complete,
            stage: UnlockPdfStage::Completed,
            page_count: result.page_count,
            output_path: Some(result.output_path.to_string_lossy().into_owned()),
            has_warnings: result.has_warnings,
            error: None,
        },
        Err(failure) => UnlockPdfUpdate {
            status: PdfSecurityStatus::Failed,
            stage: last_stage,
            page_count: 0,
            output_path: None,
            has_warnings: false,
            error: Some(failure.error.into()),
        },
    };
    let _ = progress_sink.add(update);
}

impl From<ilikepdf_core::PdfEncryptionState> for PdfEncryptionState {
    fn from(value: ilikepdf_core::PdfEncryptionState) -> Self {
        match value {
            ilikepdf_core::PdfEncryptionState::Unencrypted => Self::Unencrypted,
            ilikepdf_core::PdfEncryptionState::EncryptedNoPasswordRequired => {
                Self::EncryptedNoPasswordRequired
            }
            ilikepdf_core::PdfEncryptionState::EncryptedPasswordRequired => {
                Self::EncryptedPasswordRequired
            }
        }
    }
}

impl From<ilikepdf_core::ProtectPdfStage> for ProtectPdfStage {
    fn from(value: ilikepdf_core::ProtectPdfStage) -> Self {
        match value {
            ilikepdf_core::ProtectPdfStage::Preparing => Self::Preparing,
            ilikepdf_core::ProtectPdfStage::Protecting => Self::Protecting,
            ilikepdf_core::ProtectPdfStage::Validating => Self::Validating,
            ilikepdf_core::ProtectPdfStage::Publishing => Self::Publishing,
            ilikepdf_core::ProtectPdfStage::Completed => Self::Completed,
        }
    }
}

impl From<ilikepdf_core::UnlockPdfStage> for UnlockPdfStage {
    fn from(value: ilikepdf_core::UnlockPdfStage) -> Self {
        match value {
            ilikepdf_core::UnlockPdfStage::Preparing => Self::Preparing,
            ilikepdf_core::UnlockPdfStage::Unlocking => Self::Unlocking,
            ilikepdf_core::UnlockPdfStage::Validating => Self::Validating,
            ilikepdf_core::UnlockPdfStage::Publishing => Self::Publishing,
            ilikepdf_core::UnlockPdfStage::Completed => Self::Completed,
        }
    }
}
