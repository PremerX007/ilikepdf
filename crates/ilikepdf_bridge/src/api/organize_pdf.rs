use std::path::PathBuf;

use crate::frb_generated::StreamSink;

use super::application::ApplicationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizePdfSourceInfo {
    pub source_path: String,
    pub page_count: u32,
    pub has_warnings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizePdfSource {
    pub source_id: u32,
    pub source_path: String,
    pub page_count: u32,
    pub has_warnings: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizePdfPageRotation {
    None,
    Clockwise90,
    HalfTurn,
    CounterClockwise90,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizePdfPageItem {
    pub page_item_id: u32,
    pub source_id: u32,
    pub source_page_index: u32,
    pub rotation: OrganizePdfPageRotation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizePdfRequest {
    pub sources: Vec<OrganizePdfSource>,
    pub page_items: Vec<OrganizePdfPageItem>,
    pub destination_directory: String,
    pub output_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizePdfStage {
    Preparing,
    Organizing,
    Validating,
    Publishing,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizePdfStatus {
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizePdfUpdate {
    pub status: OrganizePdfStatus,
    pub stage: OrganizePdfStage,
    pub source_count: u32,
    pub page_count: u32,
    pub output_path: Option<String>,
    pub warning_source_count: u32,
    pub has_warnings: bool,
    pub failed_source_id: Option<u32>,
    pub failed_source_path: Option<String>,
    pub failed_page_item_id: Option<u32>,
    pub error: Option<ApplicationError>,
}

/// Validates a candidate group without mutating the caller's existing session.
/// The returned additions are all valid; any failure rejects the whole group.
pub fn inspect_organize_pdf_sources(
    existing_source_paths: Vec<String>,
    candidate_source_paths: Vec<String>,
) -> Result<Vec<OrganizePdfSourceInfo>, ApplicationError> {
    crate::logging::record(crate::logging::Event::OrganizePdfInspectionRequested);
    let engine = ilikepdf_qpdf::QpdfCliEngine::bundled();
    let existing = existing_source_paths
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let candidates = candidate_source_paths
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    ilikepdf_core::inspect_organize_pdf_sources(&engine, &existing, &candidates)
        .map(|sources| {
            sources
                .into_iter()
                .map(|source| OrganizePdfSourceInfo {
                    source_path: source.source_path.to_string_lossy().into_owned(),
                    page_count: source.page_count,
                    has_warnings: source.has_warnings,
                })
                .collect()
        })
        .map_err(Into::into)
}

/// Organizes one typed multi-source page plan on the native worker pool.
pub fn organize_pdf(request: OrganizePdfRequest, progress_sink: StreamSink<OrganizePdfUpdate>) {
    crate::logging::record(crate::logging::Event::OrganizePdfRequested);
    let engine = ilikepdf_qpdf::QpdfCliEngine::bundled();
    let mut last_stage = OrganizePdfStage::Preparing;
    let result = ilikepdf_core::organize_pdf(
        &engine,
        ilikepdf_core::OrganizePdfRequest {
            sources: request
                .sources
                .into_iter()
                .map(|source| ilikepdf_core::OrganizePdfSource {
                    source_id: source.source_id,
                    source_path: PathBuf::from(source.source_path),
                    page_count: source.page_count,
                    has_warnings: source.has_warnings,
                })
                .collect(),
            page_items: request
                .page_items
                .into_iter()
                .map(|page| ilikepdf_core::OrganizePdfPageItem {
                    page_item_id: page.page_item_id,
                    source_id: page.source_id,
                    source_page_index: page.source_page_index,
                    rotation: page.rotation.into(),
                })
                .collect(),
            destination_directory: PathBuf::from(request.destination_directory),
            output_name: request.output_name,
        },
        |progress| {
            last_stage = progress.stage.into();
            let _ = progress_sink.add(OrganizePdfUpdate {
                status: OrganizePdfStatus::Running,
                stage: last_stage,
                source_count: progress.source_count,
                page_count: progress.page_count,
                output_path: None,
                warning_source_count: 0,
                has_warnings: false,
                failed_source_id: None,
                failed_source_path: None,
                failed_page_item_id: None,
                error: None,
            });
        },
    );

    let update = match result {
        Ok(result) => OrganizePdfUpdate {
            status: OrganizePdfStatus::Complete,
            stage: OrganizePdfStage::Completed,
            source_count: result.source_count,
            page_count: result.page_count,
            output_path: Some(result.output_path.to_string_lossy().into_owned()),
            warning_source_count: result.warning_source_count,
            has_warnings: result.has_warnings,
            failed_source_id: None,
            failed_source_path: None,
            failed_page_item_id: None,
            error: None,
        },
        Err(failure) => OrganizePdfUpdate {
            status: OrganizePdfStatus::Failed,
            stage: last_stage,
            source_count: failure.source_count,
            page_count: failure.page_count,
            output_path: None,
            warning_source_count: 0,
            has_warnings: false,
            failed_source_id: failure.source_id,
            failed_source_path: failure
                .source_path
                .map(|path| path.to_string_lossy().into_owned()),
            failed_page_item_id: failure.page_item_id,
            error: Some(failure.error.into()),
        },
    };
    let _ = progress_sink.add(update);
}

impl From<OrganizePdfPageRotation> for ilikepdf_core::OrganizePdfPageRotation {
    fn from(value: OrganizePdfPageRotation) -> Self {
        match value {
            OrganizePdfPageRotation::None => Self::None,
            OrganizePdfPageRotation::Clockwise90 => Self::Clockwise90,
            OrganizePdfPageRotation::HalfTurn => Self::HalfTurn,
            OrganizePdfPageRotation::CounterClockwise90 => Self::CounterClockwise90,
        }
    }
}

impl From<ilikepdf_core::OrganizePdfStage> for OrganizePdfStage {
    fn from(value: ilikepdf_core::OrganizePdfStage) -> Self {
        match value {
            ilikepdf_core::OrganizePdfStage::Preparing => Self::Preparing,
            ilikepdf_core::OrganizePdfStage::Organizing => Self::Organizing,
            ilikepdf_core::OrganizePdfStage::Validating => Self::Validating,
            ilikepdf_core::OrganizePdfStage::Publishing => Self::Publishing,
            ilikepdf_core::OrganizePdfStage::Completed => Self::Completed,
        }
    }
}
