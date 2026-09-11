use std::fs;
use std::path::{Path, PathBuf};

use tempfile::{Builder, NamedTempFile};

use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

pub(crate) struct PendingPngOutput {
    pub(crate) file: NamedTempFile,
    destination: Option<PathBuf>,
}

impl PendingPngOutput {
    pub(crate) fn create(destination: Option<&Path>) -> ApplicationResult<Self> {
        match destination {
            Some(path) => Self::for_destination(path),
            None => Self::for_preview(),
        }
    }

    pub(crate) fn for_destination(path: &Path) -> ApplicationResult<Self> {
        if !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
        {
            return Err(ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "The output destination must use a .png extension",
            ));
        }
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
            destination: Some(path.to_path_buf()),
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
            destination: None,
        })
    }

    pub(crate) fn publish(self) -> ApplicationResult<PathBuf> {
        self.file
            .as_file()
            .sync_all()
            .map_err(map_output_io_error)?;

        match self.destination {
            Some(destination) => self
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
            None => self
                .file
                .keep()
                .map(|(_, path)| path)
                .map_err(|error| map_output_io_error(error.error)),
        }
    }
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
mod tests {
    use super::*;

    #[test]
    fn rejects_non_png_destinations() {
        let error = PendingPngOutput::create(Some(Path::new("preview.jpg")))
            .err()
            .expect("non-PNG destinations should be rejected");

        assert_eq!(error.code, ApplicationErrorCode::InvalidRequest);
    }

    #[test]
    fn refuses_to_replace_an_existing_output() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let destination = directory.path().join("existing.png");
        fs::write(&destination, b"existing content").expect("test output should be created");

        let error = PendingPngOutput::create(Some(&destination))
            .err()
            .expect("existing output should be rejected");

        assert_eq!(error.code, ApplicationErrorCode::OutputAlreadyExists);
        assert_eq!(
            fs::read(destination).expect("existing output should remain readable"),
            b"existing content"
        );
    }
}
