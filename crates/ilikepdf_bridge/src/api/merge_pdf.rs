use std::path::PathBuf;

use crate::frb_generated::StreamSink;

use super::application::ApplicationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePdfRequest {
    pub source_paths: Vec<String>,
    pub destination_directory: String,
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
pub enum MergePdfStatus {
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePdfUpdate {
    pub status: MergePdfStatus,
    pub stage: MergePdfStage,
    pub input_count: u32,
    pub total_page_count: u32,
    pub output_path: Option<String>,
    pub warning_input_count: u32,
    pub has_warnings: bool,
    pub failed_input_index: Option<u32>,
    pub failed_input_path: Option<String>,
    pub error: Option<ApplicationError>,
}

/// Merges PDFs on flutter_rust_bridge's worker pool and streams real job stages.
pub fn merge_pdf(request: MergePdfRequest, progress_sink: StreamSink<MergePdfUpdate>) {
    crate::logging::record(crate::logging::Event::MergePdfRequested);
    let engine = ilikepdf_qpdf::QpdfCliEngine::bundled();
    let mut last_stage = MergePdfStage::Preparing;
    let result = ilikepdf_core::merge_pdf(
        &engine,
        ilikepdf_core::MergePdfRequest {
            source_paths: request
                .source_paths
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            destination_directory: PathBuf::from(request.destination_directory),
            output_name: request.output_name,
        },
        |progress| {
            last_stage = progress.stage.into();
            let _ = progress_sink.add(MergePdfUpdate {
                status: MergePdfStatus::Running,
                stage: last_stage,
                input_count: progress.input_count,
                total_page_count: progress.total_page_count,
                output_path: None,
                warning_input_count: 0,
                has_warnings: false,
                failed_input_index: None,
                failed_input_path: None,
                error: None,
            });
        },
    );

    let update = match result {
        Ok(result) => MergePdfUpdate {
            status: MergePdfStatus::Complete,
            stage: MergePdfStage::Completed,
            input_count: result.input_count,
            total_page_count: result.page_count,
            output_path: Some(result.output_path.to_string_lossy().into_owned()),
            warning_input_count: result.warning_input_count,
            has_warnings: result.has_warnings,
            failed_input_index: None,
            failed_input_path: None,
            error: None,
        },
        Err(failure) => MergePdfUpdate {
            status: MergePdfStatus::Failed,
            stage: last_stage,
            input_count: failure.input_count,
            total_page_count: failure.expected_page_count,
            output_path: None,
            warning_input_count: 0,
            has_warnings: false,
            failed_input_index: failure.input_index,
            failed_input_path: failure
                .input_path
                .map(|path| path.to_string_lossy().into_owned()),
            error: Some(failure.error.into()),
        },
    };
    let _ = progress_sink.add(update);
}

impl From<ilikepdf_core::MergePdfStage> for MergePdfStage {
    fn from(value: ilikepdf_core::MergePdfStage) -> Self {
        match value {
            ilikepdf_core::MergePdfStage::Preparing => Self::Preparing,
            ilikepdf_core::MergePdfStage::Merging => Self::Merging,
            ilikepdf_core::MergePdfStage::Validating => Self::Validating,
            ilikepdf_core::MergePdfStage::Publishing => Self::Publishing,
            ilikepdf_core::MergePdfStage::Completed => Self::Completed,
        }
    }
}
