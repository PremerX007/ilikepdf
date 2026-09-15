use std::path::{Path, PathBuf};

use crate::frb_generated::StreamSink;

use super::application::ApplicationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPdfSourceInfo {
    pub page_count: u32,
    pub size_bytes: u64,
    pub has_warnings: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitPdfMode {
    EveryPage,
    EveryNPages,
    SplitAfterPages,
    MaximumFileSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPdfRequest {
    pub source_path: String,
    pub destination_directory: String,
    pub mode: SplitPdfMode,
    pub every_n_pages: Option<u32>,
    pub split_after_pages: Option<String>,
    pub maximum_size_mb: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitPdfStage {
    Preparing,
    FindingSplitPoints,
    Creating,
    Validating,
    Publishing,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitPdfStatus {
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPdfPart {
    pub output_path: String,
    pub first_page: u32,
    pub last_page: u32,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPdfUpdate {
    pub status: SplitPdfStatus,
    pub stage: SplitPdfStage,
    pub source_page_count: u32,
    pub current_part: Option<u32>,
    pub total_parts: Option<u32>,
    pub output_directory: Option<String>,
    pub parts: Vec<SplitPdfPart>,
    pub has_warnings: bool,
    pub failed_page_number: Option<u32>,
    pub actual_size_bytes: Option<u64>,
    pub limit_size_bytes: Option<u64>,
    pub error: Option<ApplicationError>,
}

pub fn inspect_split_pdf_source(
    source_path: String,
) -> Result<SplitPdfSourceInfo, ApplicationError> {
    crate::logging::record(crate::logging::Event::SplitPdfInspectionRequested);
    let engine = ilikepdf_qpdf::QpdfCliEngine::bundled();
    ilikepdf_core::inspect_split_pdf_source(&engine, Path::new(&source_path))
        .map(|info| SplitPdfSourceInfo {
            page_count: info.page_count,
            size_bytes: info.size_bytes,
            has_warnings: info.has_warnings,
        })
        .map_err(Into::into)
}

pub fn split_pdf(request: SplitPdfRequest, progress_sink: StreamSink<SplitPdfUpdate>) {
    crate::logging::record(crate::logging::Event::SplitPdfRequested);
    let mut last_stage = SplitPdfStage::Preparing;
    let mode = match to_core_mode(&request) {
        Ok(mode) => mode,
        Err(error) => {
            let _ = progress_sink.add(failed_update(last_stage, 0, error));
            return;
        }
    };
    let engine = ilikepdf_qpdf::QpdfCliEngine::bundled();
    let result = ilikepdf_core::split_pdf(
        &engine,
        ilikepdf_core::SplitPdfRequest {
            source_path: PathBuf::from(request.source_path),
            destination_directory: PathBuf::from(request.destination_directory),
            mode,
        },
        |progress| {
            last_stage = progress.stage.into();
            let _ = progress_sink.add(SplitPdfUpdate {
                status: SplitPdfStatus::Running,
                stage: last_stage,
                source_page_count: progress.source_page_count,
                current_part: progress.current_part,
                total_parts: progress.total_parts,
                output_directory: None,
                parts: Vec::new(),
                has_warnings: false,
                failed_page_number: None,
                actual_size_bytes: None,
                limit_size_bytes: None,
                error: None,
            });
        },
    );

    let update = match result {
        Ok(result) => SplitPdfUpdate {
            status: SplitPdfStatus::Complete,
            stage: SplitPdfStage::Completed,
            source_page_count: result.source_page_count,
            current_part: u32::try_from(result.parts.len()).ok(),
            total_parts: u32::try_from(result.parts.len()).ok(),
            output_directory: Some(result.output_directory.to_string_lossy().into_owned()),
            parts: result
                .parts
                .into_iter()
                .map(|part| SplitPdfPart {
                    output_path: part.output_path.to_string_lossy().into_owned(),
                    first_page: part.range.first_page,
                    last_page: part.range.last_page,
                    size_bytes: part.size_bytes,
                })
                .collect(),
            has_warnings: result.has_warnings,
            failed_page_number: None,
            actual_size_bytes: None,
            limit_size_bytes: None,
            error: None,
        },
        Err(failure) => SplitPdfUpdate {
            status: SplitPdfStatus::Failed,
            stage: last_stage,
            source_page_count: failure.source_page_count,
            current_part: None,
            total_parts: None,
            output_directory: None,
            parts: Vec::new(),
            has_warnings: false,
            failed_page_number: failure.page_number,
            actual_size_bytes: failure.actual_size_bytes,
            limit_size_bytes: failure.limit_size_bytes,
            error: Some(failure.error.into()),
        },
    };
    let _ = progress_sink.add(update);
}

fn to_core_mode(
    request: &SplitPdfRequest,
) -> Result<ilikepdf_core::SplitPdfMode, ApplicationError> {
    let invalid = || ApplicationError {
        code: super::error::ApplicationErrorCode::InvalidSplitConfiguration,
        message: "The selected split settings are incomplete".to_owned(),
    };
    match request.mode {
        SplitPdfMode::EveryPage => Ok(ilikepdf_core::SplitPdfMode::EveryPage),
        SplitPdfMode::EveryNPages => Ok(ilikepdf_core::SplitPdfMode::EveryNPages {
            pages_per_part: request.every_n_pages.ok_or_else(invalid)?,
        }),
        SplitPdfMode::SplitAfterPages => Ok(ilikepdf_core::SplitPdfMode::SplitAfterPages {
            split_after_pages: request.split_after_pages.clone().ok_or_else(invalid)?,
        }),
        SplitPdfMode::MaximumFileSize => Ok(ilikepdf_core::SplitPdfMode::MaximumFileSize {
            maximum_size_mb: request.maximum_size_mb.ok_or_else(invalid)?,
        }),
    }
}

fn failed_update(
    stage: SplitPdfStage,
    source_page_count: u32,
    error: ApplicationError,
) -> SplitPdfUpdate {
    SplitPdfUpdate {
        status: SplitPdfStatus::Failed,
        stage,
        source_page_count,
        current_part: None,
        total_parts: None,
        output_directory: None,
        parts: Vec::new(),
        has_warnings: false,
        failed_page_number: None,
        actual_size_bytes: None,
        limit_size_bytes: None,
        error: Some(error),
    }
}

impl From<ilikepdf_core::SplitPdfStage> for SplitPdfStage {
    fn from(value: ilikepdf_core::SplitPdfStage) -> Self {
        match value {
            ilikepdf_core::SplitPdfStage::Preparing => Self::Preparing,
            ilikepdf_core::SplitPdfStage::FindingSplitPoints => Self::FindingSplitPoints,
            ilikepdf_core::SplitPdfStage::Creating => Self::Creating,
            ilikepdf_core::SplitPdfStage::Validating => Self::Validating,
            ilikepdf_core::SplitPdfStage::Publishing => Self::Publishing,
            ilikepdf_core::SplitPdfStage::Completed => Self::Completed,
        }
    }
}
