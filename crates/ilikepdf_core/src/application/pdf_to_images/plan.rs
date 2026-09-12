use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use super::model::PdfExportFormat;
use crate::application::output::{collision_key, occupied_names};
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

pub(super) struct PdfExportOutputPlan {
    pub(super) destinations: Vec<PathBuf>,
    pub(super) number_single_file: bool,
}

pub(super) fn output_plan(
    source_path: &Path,
    selected_directory: &Path,
    page_count: u32,
    format: PdfExportFormat,
) -> ApplicationResult<PdfExportOutputPlan> {
    if page_count == 0 {
        return Ok(PdfExportOutputPlan {
            destinations: Vec::new(),
            number_single_file: false,
        });
    }

    let source_stem = source_path
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| {
            ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "The source PDF must have a file name",
            )
        })?;
    let (output_directory, number_single_file) = if page_count == 1 {
        (selected_directory.to_path_buf(), true)
    } else {
        (
            create_numbered_output_directory(selected_directory, source_stem)?,
            false,
        )
    };
    let padding = page_count.to_string().len().max(4);
    let destinations = (1..=page_count)
        .map(|page_number| {
            output_directory.join(output_file_name(
                source_stem,
                page_number,
                padding,
                format.extension(),
            ))
        })
        .collect();

    Ok(PdfExportOutputPlan {
        destinations,
        number_single_file,
    })
}

fn output_file_name(
    source_stem: &OsStr,
    page_number: u32,
    padding: usize,
    extension: &str,
) -> OsString {
    let mut name = source_stem.to_os_string();
    name.push(format!("-page-{page_number:0padding$}.{extension}"));
    name
}

pub(super) fn create_numbered_output_directory(
    base: &Path,
    stem: &OsStr,
) -> ApplicationResult<PathBuf> {
    let mut occupied_names = occupied_names(base).map_err(map_directory_error)?;
    for number in 0..=u32::MAX {
        let folder_name = numbered_name(stem, number);
        if !occupied_names.insert(collision_key(&folder_name)) {
            continue;
        }
        let candidate = base.join(folder_name);
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(map_directory_error(error)),
        }
    }

    Err(ApplicationError::new(
        ApplicationErrorCode::OutputWriteFailed,
        "A safe output folder name could not be allocated",
    ))
}

fn numbered_name(stem: &OsStr, number: u32) -> OsString {
    let mut name = stem.to_os_string();
    if number > 0 {
        name.push(format!(" ({number})"));
    }
    name
}

fn map_directory_error(error: std::io::Error) -> ApplicationError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => ApplicationError::new(
            ApplicationErrorCode::PermissionDenied,
            "Permission was denied while creating the output folder",
        ),
        _ => ApplicationError::new(
            ApplicationErrorCode::OutputWriteFailed,
            "The output folder could not be created",
        ),
    }
}
