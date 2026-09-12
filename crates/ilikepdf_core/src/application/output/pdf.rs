use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use tempfile::{Builder, NamedTempFile, TempPath};

use super::image::validate_output_directory;
use super::publication::{NumberedPublicationError, publish_numbered};
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
