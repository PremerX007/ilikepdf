use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::{Builder, NamedTempFile};

use super::publication::{NumberedPublicationError, publish_numbered as publish_numbered_file};
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

pub(crate) struct PendingImageOutput {
    pub(crate) file: NamedTempFile,
    destination: ImageDestination,
}

enum ImageDestination {
    Preview,
    Exact(PathBuf),
    Numbered(PathBuf),
}

impl PendingImageOutput {
    pub(crate) fn create_png(destination: Option<&Path>) -> ApplicationResult<Self> {
        match destination {
            Some(path) => Self::for_destination(path, "png"),
            None => Self::for_preview(),
        }
    }

    pub(crate) fn for_destination(
        path: &Path,
        expected_extension: &str,
    ) -> ApplicationResult<Self> {
        validate_image_path(path, expected_extension)?;
        if path.exists() {
            return Err(ApplicationError::new(
                ApplicationErrorCode::OutputAlreadyExists,
                "An output image already exists",
            ));
        }

        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty());
        let parent = match parent {
            Some(parent) => parent.to_path_buf(),
            None => std::env::current_dir().map_err(map_output_io_error)?,
        };
        validate_output_directory(&parent)?;

        let file = Builder::new()
            .prefix(".ilikepdf-render-")
            .suffix(".tmp")
            .tempfile_in(parent)
            .map_err(map_output_io_error)?;

        Ok(Self {
            file,
            destination: ImageDestination::Exact(path.to_path_buf()),
        })
    }

    pub(crate) fn for_numbered_destination(
        path: &Path,
        expected_extension: &str,
    ) -> ApplicationResult<Self> {
        validate_image_path(path, expected_extension)?;
        let parent = output_parent(path)?;
        validate_output_directory(&parent)?;
        let file = Builder::new()
            .prefix(".ilikepdf-render-")
            .suffix(".tmp")
            .tempfile_in(parent)
            .map_err(map_output_io_error)?;

        Ok(Self {
            file,
            destination: ImageDestination::Numbered(path.to_path_buf()),
        })
    }

    fn for_preview() -> ApplicationResult<Self> {
        let directory = std::env::temp_dir().join("ilikepdf").join("previews");
        fs::create_dir_all(&directory).map_err(map_output_io_error)?;
        let file = Builder::new()
            .prefix("preview-")
            .suffix(".png")
            .tempfile_in(directory)
            .map_err(map_output_io_error)?;

        Ok(Self {
            file,
            destination: ImageDestination::Preview,
        })
    }

    pub(crate) fn publish(self) -> ApplicationResult<PathBuf> {
        self.file
            .as_file()
            .sync_all()
            .map_err(map_output_io_error)?;

        match self.destination {
            ImageDestination::Exact(destination) => self
                .file
                .persist_noclobber(&destination)
                .map(|_| destination)
                .map_err(|error| {
                    if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                        ApplicationError::new(
                            ApplicationErrorCode::OutputAlreadyExists,
                            "An output image already exists",
                        )
                    } else {
                        map_output_io_error(error.error)
                    }
                }),
            ImageDestination::Numbered(destination) => publish_numbered(self.file, &destination),
            ImageDestination::Preview => self
                .file
                .keep()
                .map(|(_, path)| path)
                .map_err(|error| map_output_io_error(error.error)),
        }
    }
}

fn validate_image_path(path: &Path, expected_extension: &str) -> ApplicationResult<()> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected_extension))
    {
        Ok(())
    } else {
        Err(ApplicationError::new(
            ApplicationErrorCode::InvalidRequest,
            "The output destination has the wrong image extension",
        ))
    }
}

fn output_parent(path: &Path) -> ApplicationResult<PathBuf> {
    match path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        Some(parent) => Ok(parent.to_path_buf()),
        None => std::env::current_dir().map_err(map_output_io_error),
    }
}

fn publish_numbered(file: NamedTempFile, desired: &Path) -> ApplicationResult<PathBuf> {
    let parent = output_parent(desired)?;
    let stem = desired
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| {
            ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "The output image must have a file name",
            )
        })?;
    let extension = desired.extension().ok_or_else(|| {
        ApplicationError::new(
            ApplicationErrorCode::InvalidRequest,
            "The output image must have a file extension",
        )
    })?;
    publish_numbered_file(file, &parent, |number| {
        numbered_image_name(stem, extension, number)
    })
    .map_err(|error| match error {
        NumberedPublicationError::Io(error) => map_output_io_error(error),
        NumberedPublicationError::Exhausted => ApplicationError::new(
            ApplicationErrorCode::OutputWriteFailed,
            "A safe output image name could not be allocated",
        ),
    })
}

fn numbered_image_name(stem: &OsStr, extension: &OsStr, number: u32) -> OsString {
    let mut name = stem.to_os_string();
    if number > 0 {
        name.push(format!(" ({number})"));
    }
    name.push(".");
    name.push(extension);
    name
}

pub(crate) fn validate_output_directory(directory: &Path) -> ApplicationResult<()> {
    match fs::metadata(directory) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(invalid_output_directory()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(permission_denied())
        }
        Err(_) => Err(invalid_output_directory()),
    }
}

pub(crate) fn validate_writable_output_directory(directory: &Path) -> ApplicationResult<()> {
    validate_output_directory(directory)?;
    Builder::new()
        .prefix(".ilikepdf-write-check-")
        .suffix(".tmp")
        .tempfile_in(directory)
        .map_err(map_output_io_error)?
        .close()
        .map_err(map_output_io_error)
}

fn invalid_output_directory() -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::InvalidOutputDirectory,
        "Choose an existing output directory",
    )
}

fn permission_denied() -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::PermissionDenied,
        "Permission was denied while writing the output image",
    )
}

fn map_output_io_error(error: std::io::Error) -> ApplicationError {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        permission_denied()
    } else {
        ApplicationError::new(
            ApplicationErrorCode::OutputWriteFailed,
            "The output image could not be written",
        )
    }
}

#[cfg(test)]
mod tests;
