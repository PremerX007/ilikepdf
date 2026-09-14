use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use tempfile::{Builder, NamedTempFile, TempPath};

use super::image::validate_output_directory;
use super::publication::{
    NumberedPublicationError, collision_key, occupied_names, publish_numbered,
};
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

pub(crate) struct PendingPdfOutput {
    pub(crate) file: NamedTempFile,
    destination_directory: PathBuf,
}

#[derive(Debug)]
pub(crate) struct PendingExactPdfOutput {
    path: TempPath,
    destination: PathBuf,
}

#[derive(Debug)]
pub(crate) struct PendingNumberedPdfOutput {
    path: TempPath,
    destination_directory: PathBuf,
    output_stem: OsString,
}

impl PendingPdfOutput {
    pub(crate) fn in_directory(destination_directory: &Path) -> ApplicationResult<Self> {
        validate_output_directory(destination_directory)?;
        let file = Builder::new()
            .prefix(".ilikepdf-image-pdf-")
            .suffix(".tmp")
            .tempfile_in(destination_directory)
            .map_err(map_output_io_error)?;

        Ok(Self {
            file,
            destination_directory: destination_directory.to_path_buf(),
        })
    }

    pub(crate) fn publish_with_stem(self, stem: &OsStr) -> ApplicationResult<PathBuf> {
        if stem.is_empty() {
            return Err(ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "The output PDF must have a file name",
            ));
        }

        self.file
            .as_file()
            .sync_all()
            .map_err(map_output_io_error)?;

        publish_numbered(self.file, &self.destination_directory, |number| {
            numbered_pdf_name(stem, number)
        })
        .map_err(|error| match error {
            NumberedPublicationError::Io(error) => map_output_io_error(error),
            NumberedPublicationError::Exhausted => ApplicationError::new(
                ApplicationErrorCode::OutputWriteFailed,
                "A safe output PDF name could not be allocated",
            ),
        })
    }
}

impl PendingExactPdfOutput {
    pub(crate) fn for_destination(destination: &Path) -> ApplicationResult<Self> {
        if !destination
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
        {
            return Err(ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "The output destination must be a PDF file",
            ));
        }
        if destination.exists() {
            return Err(ApplicationError::new(
                ApplicationErrorCode::OutputAlreadyExists,
                "An output PDF already exists",
            ));
        }

        let directory = destination
            .parent()
            .filter(|directory| !directory.as_os_str().is_empty())
            .ok_or_else(|| {
                ApplicationError::new(
                    ApplicationErrorCode::InvalidOutputDirectory,
                    "Choose an existing output directory",
                )
            })?;
        validate_output_directory(directory)?;
        let path = Builder::new()
            .prefix(".ilikepdf-structural-pdf-")
            .suffix(".tmp")
            .tempfile_in(directory)
            .map_err(map_output_io_error)?
            .into_temp_path();

        Ok(Self {
            path,
            destination: destination.to_path_buf(),
        })
    }

    pub(crate) fn working_path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn publish(self) -> ApplicationResult<PathBuf> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)
            .map_err(map_output_io_error)?
            .sync_all()
            .map_err(map_output_io_error)?;
        let destination = self.destination;
        self.path
            .persist_noclobber(&destination)
            .map(|()| destination)
            .map_err(|error| {
                if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                    ApplicationError::new(
                        ApplicationErrorCode::OutputAlreadyExists,
                        "An output PDF already exists",
                    )
                } else {
                    map_output_io_error(error.error)
                }
            })
    }
}

impl PendingNumberedPdfOutput {
    pub(crate) fn in_directory(
        destination_directory: &Path,
        output_name: &str,
    ) -> ApplicationResult<Self> {
        validate_output_directory(destination_directory)?;
        let normalized_name = normalize_pdf_filename(output_name)?;
        let output_stem = Path::new(&normalized_name)
            .file_stem()
            .expect("a validated PDF name always has a stem")
            .to_os_string();
        let path = Builder::new()
            .prefix(".ilikepdf-merge-pdf-")
            .suffix(".tmp")
            .tempfile_in(destination_directory)
            .map_err(map_output_io_error)?
            .into_temp_path();

        Ok(Self {
            path,
            destination_directory: destination_directory.to_path_buf(),
            output_stem,
        })
    }

    pub(crate) fn working_path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn publish(self) -> ApplicationResult<PathBuf> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)
            .map_err(map_output_io_error)?
            .sync_all()
            .map_err(map_output_io_error)?;

        let mut path = self.path;
        let mut occupied =
            occupied_names(&self.destination_directory).map_err(map_output_io_error)?;
        for number in 0..=u32::MAX {
            let file_name = numbered_pdf_name(&self.output_stem, number);
            if !occupied.insert(collision_key(&file_name)) {
                continue;
            }
            let destination = self.destination_directory.join(file_name);
            match path.persist_noclobber(&destination) {
                Ok(()) => return Ok(destination),
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    path = error.path;
                }
                Err(error) => return Err(map_output_io_error(error.error)),
            }
        }

        Err(ApplicationError::new(
            ApplicationErrorCode::OutputWriteFailed,
            "A safe output PDF name could not be allocated",
        ))
    }
}

fn normalize_pdf_filename(output_name: &str) -> ApplicationResult<OsString> {
    let name = output_name.trim();
    let invalid_character = name
        .chars()
        .any(|character| character.is_control() || r#"<>:"/\|?*"#.contains(character));
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with([' ', '.'])
        || invalid_character
    {
        return Err(ApplicationError::new(
            ApplicationErrorCode::InvalidRequest,
            "Enter a PDF file name without a folder path",
        ));
    }

    let normalized = if name
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("pdf"))
    {
        name.to_owned()
    } else {
        format!("{name}.pdf")
    };
    let stem = Path::new(&normalized)
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or_default();
    if stem.is_empty() || is_reserved_windows_name(stem) {
        return Err(ApplicationError::new(
            ApplicationErrorCode::InvalidRequest,
            "Enter a valid PDF file name",
        ));
    }

    Ok(OsString::from(normalized))
}

fn is_reserved_windows_name(stem: &str) -> bool {
    let basename = stem.split('.').next().unwrap_or(stem);
    matches!(
        basename.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn numbered_pdf_name(stem: &OsStr, number: u32) -> OsString {
    let mut name = stem.to_os_string();
    if number > 0 {
        name.push(format!(" ({number})"));
    }
    name.push(".pdf");
    name
}

fn map_output_io_error(error: std::io::Error) -> ApplicationError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => ApplicationError::new(
            ApplicationErrorCode::PermissionDenied,
            "Permission was denied while writing the output PDF",
        ),
        _ => ApplicationError::new(
            ApplicationErrorCode::OutputWriteFailed,
            "The output PDF could not be written",
        ),
    }
}

#[cfg(test)]
mod tests;
