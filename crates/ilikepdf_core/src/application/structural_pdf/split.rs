use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    StructuralPdfEngine, StructuralPdfError, StructuralPdfOperationResult,
    StructuralPdfPageRangeRequest,
};
use crate::application::output::PendingSplitDirectory;
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

const DECIMAL_BYTES_PER_MB: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitPdfMode {
    EveryPage,
    EveryNPages { pages_per_part: u32 },
    SplitAfterPages { split_after_pages: String },
    MaximumFileSize { maximum_size_mb: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPdfRequest {
    pub source_path: PathBuf,
    pub destination_directory: PathBuf,
    pub mode: SplitPdfMode,
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
pub struct SplitPdfProgress {
    pub stage: SplitPdfStage,
    pub source_page_count: u32,
    pub current_part: Option<u32>,
    pub total_parts: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPdfSourceInfo {
    pub page_count: u32,
    pub size_bytes: u64,
    pub has_warnings: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitPdfPageRange {
    pub first_page: u32,
    pub last_page: u32,
}

impl SplitPdfPageRange {
    pub fn page_count(self) -> u32 {
        self.last_page
            .checked_sub(self.first_page)
            .and_then(|difference| difference.checked_add(1))
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPdfPart {
    pub output_path: PathBuf,
    pub range: SplitPdfPageRange,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPdfResult {
    pub output_directory: PathBuf,
    pub source_page_count: u32,
    pub parts: Vec<SplitPdfPart>,
    pub has_warnings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPdfFailure {
    pub error: ApplicationError,
    pub source_page_count: u32,
    pub page_number: Option<u32>,
    pub actual_size_bytes: Option<u64>,
    pub limit_size_bytes: Option<u64>,
}

pub fn inspect_split_pdf_source(
    engine: &dyn StructuralPdfEngine,
    source_path: &Path,
) -> ApplicationResult<SplitPdfSourceInfo> {
    validate_source(source_path)?;
    let validation = engine
        .validate(source_path)
        .map_err(map_split_engine_error)?;
    let page_count = NativeSplitPdfVerifier.verify(source_path)?;
    let size_bytes = fs::metadata(source_path)
        .map_err(|_| source_unreadable_error())?
        .len();
    Ok(SplitPdfSourceInfo {
        page_count,
        size_bytes,
        has_warnings: validation.has_warnings,
    })
}

pub fn split_pdf(
    engine: &dyn StructuralPdfEngine,
    request: SplitPdfRequest,
    on_progress: impl FnMut(SplitPdfProgress),
) -> Result<SplitPdfResult, SplitPdfFailure> {
    split_pdf_with_verifier(engine, &NativeSplitPdfVerifier, request, on_progress)
}

fn split_pdf_with_verifier(
    engine: &dyn StructuralPdfEngine,
    verifier: &impl SplitPdfVerifier,
    request: SplitPdfRequest,
    mut on_progress: impl FnMut(SplitPdfProgress),
) -> Result<SplitPdfResult, SplitPdfFailure> {
    emit_progress(&mut on_progress, SplitPdfStage::Preparing, 0, None, None);
    validate_source(&request.source_path).map_err(SplitPdfFailure::global)?;
    let source_validation = engine
        .validate(&request.source_path)
        .map_err(map_split_engine_error)
        .map_err(SplitPdfFailure::global)?;
    let source_page_count = verifier
        .verify(&request.source_path)
        .map_err(SplitPdfFailure::global)?;
    if source_page_count < 2 {
        return Err(SplitPdfFailure::with_page_count(
            ApplicationError::new(
                ApplicationErrorCode::PdfHasTooFewPages,
                "This PDF has only one page and cannot be split",
            ),
            source_page_count,
        ));
    }
    let source_metadata = fs::metadata(&request.source_path).map_err(|_| {
        SplitPdfFailure::with_page_count(source_unreadable_error(), source_page_count)
    })?;
    let (planned_ranges, size_limit) = match &request.mode {
        SplitPdfMode::MaximumFileSize { maximum_size_mb } => {
            let limit = maximum_size_mb
                .checked_mul(DECIMAL_BYTES_PER_MB)
                .filter(|limit| *limit > 0)
                .ok_or_else(|| {
                    SplitPdfFailure::with_page_count(
                        invalid_configuration("Enter a positive whole-number maximum size in MB"),
                        source_page_count,
                    )
                })?;
            if source_metadata.len() <= limit {
                return Err(SplitPdfFailure::with_page_count(
                    ApplicationError::new(
                        ApplicationErrorCode::SplitNotRequired,
                        "This PDF is already below the selected size limit",
                    ),
                    source_page_count,
                ));
            }
            (None, Some(limit))
        }
        mode => (
            Some(
                plan_deterministic_ranges(source_page_count, mode)
                    .map_err(|error| SplitPdfFailure::with_page_count(error, source_page_count))?,
            ),
            None,
        ),
    };

    let source_stem = request
        .source_path
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| {
            SplitPdfFailure::with_page_count(
                ApplicationError::new(
                    ApplicationErrorCode::InvalidRequest,
                    "The source PDF must have a file name",
                ),
                source_page_count,
            )
        })?;
    let pending = PendingSplitDirectory::in_directory(&request.destination_directory, source_stem)
        .map_err(|error| SplitPdfFailure::with_page_count(error, source_page_count))?;

    let mut working_parts = if let Some(ranges) = planned_ranges {
        create_planned_parts(
            engine,
            &request.source_path,
            pending.working_path(),
            source_stem,
            &request.mode,
            ranges,
            source_page_count,
            &mut on_progress,
        )?
    } else {
        let limit = size_limit.ok_or_else(|| {
            SplitPdfFailure::with_page_count(
                invalid_configuration("Enter a maximum file size"),
                source_page_count,
            )
        })?;
        create_size_bounded_parts(
            engine,
            &request.source_path,
            pending.working_path(),
            source_stem,
            source_page_count,
            limit,
            &mut on_progress,
        )?
    };

    if working_parts.len() < 2 {
        return Err(SplitPdfFailure::with_page_count(
            ApplicationError::new(
                ApplicationErrorCode::SplitNotRequired,
                "The selected settings would not split this PDF into multiple files",
            ),
            source_page_count,
        ));
    }

    let total_parts = u32::try_from(working_parts.len()).unwrap_or(u32::MAX);
    let mut has_warnings = source_validation.has_warnings;
    for (index, part) in working_parts.iter_mut().enumerate() {
        emit_progress(
            &mut on_progress,
            SplitPdfStage::Validating,
            source_page_count,
            u32::try_from(index + 1).ok(),
            Some(total_parts),
        );
        let metadata = fs::metadata(&part.working_path).map_err(|_| {
            SplitPdfFailure::with_page_count(output_validation_error(), source_page_count)
        })?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(SplitPdfFailure::with_page_count(
                output_validation_error(),
                source_page_count,
            ));
        }
        let validation = engine.validate(&part.working_path).map_err(|_| {
            SplitPdfFailure::with_page_count(
                ApplicationError::new(
                    ApplicationErrorCode::StructuralPdfOutputValidationFailed,
                    "A split output failed structural validation",
                ),
                source_page_count,
            )
        })?;
        let actual_page_count = verifier.verify(&part.working_path).map_err(|_| {
            SplitPdfFailure::with_page_count(
                ApplicationError::new(
                    ApplicationErrorCode::StructuralPdfOutputValidationFailed,
                    "A split output could not be reopened",
                ),
                source_page_count,
            )
        })?;
        if actual_page_count != part.range.page_count() {
            return Err(SplitPdfFailure::with_page_count(
                ApplicationError::new(
                    ApplicationErrorCode::SplitOutputPageCountMismatch,
                    "A split output did not contain the expected number of pages",
                ),
                source_page_count,
            ));
        }
        part.size_bytes = metadata.len();
        if let Some(limit) = size_limit
            && part.size_bytes > limit
        {
            return Err(SplitPdfFailure {
                error: ApplicationError::new(
                    ApplicationErrorCode::SplitOutputExceedsSizeLimit,
                    "A generated split output exceeded the selected size limit",
                ),
                source_page_count,
                page_number: None,
                actual_size_bytes: Some(part.size_bytes),
                limit_size_bytes: Some(limit),
            });
        }
        sync_file(&part.working_path)
            .map_err(|error| SplitPdfFailure::with_page_count(error, source_page_count))?;
        has_warnings |= part.operation.has_warnings || validation.has_warnings;
    }

    emit_progress(
        &mut on_progress,
        SplitPdfStage::Publishing,
        source_page_count,
        None,
        Some(total_parts),
    );
    let published_directory = pending
        .publish()
        .map_err(|error| SplitPdfFailure::with_page_count(error, source_page_count))?;
    let parts = working_parts
        .into_iter()
        .map(|part| SplitPdfPart {
            output_path: published_directory.join(part.file_name),
            range: part.range,
            size_bytes: part.size_bytes,
        })
        .collect();
    emit_progress(
        &mut on_progress,
        SplitPdfStage::Completed,
        source_page_count,
        Some(total_parts),
        Some(total_parts),
    );
    Ok(SplitPdfResult {
        output_directory: published_directory,
        source_page_count,
        parts,
        has_warnings,
    })
}

fn plan_deterministic_ranges(
    page_count: u32,
    mode: &SplitPdfMode,
) -> ApplicationResult<Vec<SplitPdfPageRange>> {
    match mode {
        SplitPdfMode::EveryPage => Ok((1..=page_count)
            .map(|page| SplitPdfPageRange {
                first_page: page,
                last_page: page,
            })
            .collect()),
        SplitPdfMode::EveryNPages { pages_per_part } => {
            if *pages_per_part == 0 || *pages_per_part >= page_count {
                return Err(invalid_configuration(
                    "Split every N pages must be at least 1 and less than the page count",
                ));
            }
            let mut ranges = Vec::new();
            let mut first_page = 1;
            while first_page <= page_count {
                let last_page = first_page
                    .saturating_add(*pages_per_part - 1)
                    .min(page_count);
                ranges.push(SplitPdfPageRange {
                    first_page,
                    last_page,
                });
                if last_page == page_count {
                    break;
                }
                first_page = last_page + 1;
            }
            Ok(ranges)
        }
        SplitPdfMode::SplitAfterPages { split_after_pages } => {
            let split_points = parse_split_points(split_after_pages, page_count)?;
            let mut first_page = 1;
            let mut ranges = Vec::with_capacity(split_points.len() + 1);
            for last_page in split_points {
                ranges.push(SplitPdfPageRange {
                    first_page,
                    last_page,
                });
                first_page = last_page + 1;
            }
            ranges.push(SplitPdfPageRange {
                first_page,
                last_page: page_count,
            });
            Ok(ranges)
        }
        SplitPdfMode::MaximumFileSize { .. } => Err(invalid_configuration(
            "Maximum file size ranges are discovered during splitting",
        )),
    }
}

fn parse_split_points(value: &str, page_count: u32) -> ApplicationResult<Vec<u32>> {
    if value.trim().is_empty() {
        return Err(invalid_configuration("Enter at least one split point"));
    }
    let mut points = Vec::new();
    for token in value.split(',') {
        let token = token.trim();
        if token.is_empty() {
            return Err(invalid_configuration(
                "Split points must be comma-separated whole page numbers",
            ));
        }
        let page = token.parse::<u32>().map_err(|_| {
            invalid_configuration("Split points must be comma-separated whole page numbers")
        })?;
        if page == 0 || page >= page_count {
            return Err(invalid_configuration(
                "Each split point must be between page 1 and the next-to-last page",
            ));
        }
        if points.last().is_some_and(|previous| page <= *previous) {
            return Err(invalid_configuration(
                "Split points must be strictly ascending with no duplicates",
            ));
        }
        points.push(page);
    }
    Ok(points)
}

#[allow(clippy::too_many_arguments)]
fn create_planned_parts(
    engine: &dyn StructuralPdfEngine,
    source_path: &Path,
    working_directory: &Path,
    source_stem: &OsStr,
    mode: &SplitPdfMode,
    ranges: Vec<SplitPdfPageRange>,
    source_page_count: u32,
    on_progress: &mut impl FnMut(SplitPdfProgress),
) -> Result<Vec<WorkingPart>, SplitPdfFailure> {
    let total_parts = u32::try_from(ranges.len()).unwrap_or(u32::MAX);
    let every_page = matches!(mode, SplitPdfMode::EveryPage);
    let mut parts = Vec::with_capacity(ranges.len());
    for (index, range) in ranges.into_iter().enumerate() {
        let part_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        emit_progress(
            on_progress,
            SplitPdfStage::Creating,
            source_page_count,
            Some(part_number),
            Some(total_parts),
        );
        let file_name = output_file_name(source_stem, part_number, every_page);
        let working_path = working_directory.join(&file_name);
        let operation = create_range(engine, source_path, range, &working_path)
            .map_err(|error| split_engine_failure(error, source_page_count))?;
        parts.push(WorkingPart {
            working_path,
            file_name,
            range,
            size_bytes: 0,
            operation,
        });
    }
    Ok(parts)
}

fn create_size_bounded_parts(
    engine: &dyn StructuralPdfEngine,
    source_path: &Path,
    working_directory: &Path,
    source_stem: &OsStr,
    page_count: u32,
    limit: u64,
    on_progress: &mut impl FnMut(SplitPdfProgress),
) -> Result<Vec<WorkingPart>, SplitPdfFailure> {
    let probe_path = working_directory.join(".size-probe.pdf");
    let best_path = working_directory.join(".size-best.pdf");
    let mut parts = Vec::new();
    let mut first_page = 1;

    while first_page <= page_count {
        let _ = fs::remove_file(&best_path);
        let mut best: Option<(u32, u64, StructuralPdfOperationResult)> = None;
        for last_page in first_page..=page_count {
            emit_progress(
                on_progress,
                SplitPdfStage::FindingSplitPoints,
                page_count,
                None,
                None,
            );
            let _ = fs::remove_file(&probe_path);
            let range = SplitPdfPageRange {
                first_page,
                last_page,
            };
            let operation = create_range(engine, source_path, range, &probe_path)
                .map_err(|error| split_engine_failure(error, page_count))?;
            let actual_size = fs::metadata(&probe_path)
                .map_err(|_| SplitPdfFailure::with_page_count(candidate_error(), page_count))?
                .len();
            if actual_size <= limit {
                let _ = fs::remove_file(&best_path);
                fs::rename(&probe_path, &best_path)
                    .map_err(|_| SplitPdfFailure::with_page_count(candidate_error(), page_count))?;
                best = Some((last_page, actual_size, operation));
                continue;
            }

            let _ = fs::remove_file(&probe_path);
            if last_page == first_page {
                return Err(SplitPdfFailure {
                    error: ApplicationError::with_message(
                        ApplicationErrorCode::SplitPageExceedsSizeLimit,
                        format!(
                            "Cannot split this PDF within the selected limit. Page {first_page} requires {:.1} MB by itself. Reducing it further would require PDF compression",
                            actual_size as f64 / DECIMAL_BYTES_PER_MB as f64
                        ),
                    ),
                    source_page_count: page_count,
                    page_number: Some(first_page),
                    actual_size_bytes: Some(actual_size),
                    limit_size_bytes: Some(limit),
                });
            }
            break;
        }

        let (last_page, actual_size, operation) =
            best.ok_or_else(|| SplitPdfFailure::with_page_count(candidate_error(), page_count))?;
        let part_number = u32::try_from(parts.len() + 1).unwrap_or(u32::MAX);
        emit_progress(
            on_progress,
            SplitPdfStage::Creating,
            page_count,
            Some(part_number),
            None,
        );
        let file_name = output_file_name(source_stem, part_number, false);
        let working_path = working_directory.join(&file_name);
        fs::rename(&best_path, &working_path)
            .map_err(|_| SplitPdfFailure::with_page_count(candidate_error(), page_count))?;
        parts.push(WorkingPart {
            working_path,
            file_name,
            range: SplitPdfPageRange {
                first_page,
                last_page,
            },
            size_bytes: actual_size,
            operation,
        });
        if last_page == page_count {
            break;
        }
        first_page = last_page + 1;
    }

    let _ = fs::remove_file(probe_path);
    let _ = fs::remove_file(best_path);
    Ok(parts)
}

fn create_range(
    engine: &dyn StructuralPdfEngine,
    source_path: &Path,
    range: SplitPdfPageRange,
    working_output_path: &Path,
) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
    engine.create_page_range(&StructuralPdfPageRangeRequest {
        source_path: source_path.to_path_buf(),
        first_page: range.first_page,
        last_page: range.last_page,
        working_output_path: working_output_path.to_path_buf(),
    })
}

struct WorkingPart {
    working_path: PathBuf,
    file_name: OsString,
    range: SplitPdfPageRange,
    size_bytes: u64,
    operation: StructuralPdfOperationResult,
}

trait SplitPdfVerifier {
    fn verify(&self, path: &Path) -> ApplicationResult<u32>;
}

struct NativeSplitPdfVerifier;

impl SplitPdfVerifier for NativeSplitPdfVerifier {
    fn verify(&self, path: &Path) -> ApplicationResult<u32> {
        Ok(ilikepdf_pdf::inspect_document(path)?.page_count)
    }
}

fn output_file_name(source_stem: &OsStr, number: u32, every_page: bool) -> OsString {
    let mut name = source_stem.to_os_string();
    name.push(if every_page { "-page-" } else { "-part-" });
    name.push(format!("{number:04}.pdf"));
    name
}

fn emit_progress(
    callback: &mut impl FnMut(SplitPdfProgress),
    stage: SplitPdfStage,
    source_page_count: u32,
    current_part: Option<u32>,
    total_parts: Option<u32>,
) {
    callback(SplitPdfProgress {
        stage,
        source_page_count,
        current_part,
        total_parts,
    });
}

fn validate_source(source_path: &Path) -> ApplicationResult<()> {
    match fs::metadata(source_path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(ApplicationError::new(
            ApplicationErrorCode::SourceNotFile,
            "The selected path is not a file",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(ApplicationError::new(
            ApplicationErrorCode::SourceNotFound,
            "The selected PDF no longer exists",
        )),
        Err(_) => Err(source_unreadable_error()),
    }
}

fn map_split_engine_error(error: StructuralPdfError) -> ApplicationError {
    let (code, message) = match error {
        StructuralPdfError::RuntimeUnavailable => (
            ApplicationErrorCode::StructuralPdfRuntimeUnavailable,
            "The local structural PDF runtime is unavailable",
        ),
        StructuralPdfError::RuntimeIncompatible => (
            ApplicationErrorCode::StructuralPdfRuntimeIncompatible,
            "The local structural PDF runtime is incompatible",
        ),
        StructuralPdfError::RuntimeLaunchFailed => (
            ApplicationErrorCode::StructuralPdfLaunchFailed,
            "The local structural PDF runtime could not be started",
        ),
        StructuralPdfError::SourceNotFound => (
            ApplicationErrorCode::SourceNotFound,
            "The selected PDF no longer exists",
        ),
        StructuralPdfError::SourceNotFile => (
            ApplicationErrorCode::SourceNotFile,
            "The selected path is not a file",
        ),
        StructuralPdfError::SourceUnreadable => (
            ApplicationErrorCode::SourceUnreadable,
            "The selected PDF could not be read",
        ),
        StructuralPdfError::PasswordRequired => (
            ApplicationErrorCode::PasswordRequired,
            "This PDF is password protected. Unlock it before splitting",
        ),
        StructuralPdfError::InvalidDocument => (
            ApplicationErrorCode::InvalidPdf,
            "The selected file is not a structurally valid PDF",
        ),
        StructuralPdfError::OutputWriteFailed => (
            ApplicationErrorCode::OutputWriteFailed,
            "A split PDF could not be written",
        ),
        StructuralPdfError::OperationFailed => (
            ApplicationErrorCode::StructuralPdfOperationFailed,
            "The structural PDF operation failed",
        ),
    };
    ApplicationError::new(code, message)
}

fn split_engine_failure(error: StructuralPdfError, source_page_count: u32) -> SplitPdfFailure {
    let mapped = match error {
        StructuralPdfError::OperationFailed | StructuralPdfError::OutputWriteFailed => {
            candidate_error()
        }
        other => map_split_engine_error(other),
    };
    SplitPdfFailure::with_page_count(mapped, source_page_count)
}

fn invalid_configuration(message: &'static str) -> ApplicationError {
    ApplicationError::new(ApplicationErrorCode::InvalidSplitConfiguration, message)
}

fn source_unreadable_error() -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::SourceUnreadable,
        "The selected PDF could not be read",
    )
}

fn candidate_error() -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::SplitCandidateGenerationFailed,
        "A split output could not be generated",
    )
}

fn output_validation_error() -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::StructuralPdfOutputValidationFailed,
        "A split output could not be validated",
    )
}

fn sync_file(path: &Path) -> ApplicationResult<()> {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .and_then(|file| file.sync_all())
        .map_err(|_| {
            ApplicationError::new(
                ApplicationErrorCode::OutputWriteFailed,
                "A split PDF could not be written",
            )
        })
}

impl SplitPdfFailure {
    fn global(error: ApplicationError) -> Self {
        Self::with_page_count(error, 0)
    }

    fn with_page_count(error: ApplicationError, source_page_count: u32) -> Self {
        Self {
            error,
            source_page_count,
            page_number: None,
            actual_size_bytes: None,
            limit_size_bytes: None,
        }
    }
}

#[cfg(test)]
mod tests;
