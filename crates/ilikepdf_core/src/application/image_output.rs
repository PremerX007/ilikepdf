use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::{Builder, NamedTempFile};

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

fn publish_numbered(mut file: NamedTempFile, desired: &Path) -> ApplicationResult<PathBuf> {
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
    let mut occupied_names = occupied_names(&parent)?;

    for number in 0..=u32::MAX {
        let file_name = numbered_image_name(stem, extension, number);
        if !occupied_names.insert(collision_key(&file_name)) {
            continue;
        }
        let destination = parent.join(file_name);
        match file.persist_noclobber(&destination) {
            Ok(_) => return Ok(destination),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                file = error.file;
            }
            Err(error) => return Err(map_output_io_error(error.error)),
        }
    }

    Err(ApplicationError::new(
        ApplicationErrorCode::OutputWriteFailed,
        "A safe output image name could not be allocated",
    ))
}

fn occupied_names(directory: &Path) -> ApplicationResult<HashSet<String>> {
    fs::read_dir(directory)
        .map_err(map_output_io_error)?
        .map(|entry| {
            entry
                .map(|entry| collision_key(&entry.file_name()))
                .map_err(map_output_io_error)
        })
        .collect()
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

fn collision_key(file_name: &OsStr) -> String {
    file_name.to_string_lossy().to_lowercase()
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
mod tests {
    use std::io::Write;
    use std::sync::{Arc, Barrier};
    use std::thread;

    use super::*;

    #[test]
    fn rejects_wrong_preview_extension() {
        let error = PendingImageOutput::create_png(Some(Path::new("preview.jpg")))
            .err()
            .expect("non-PNG preview destinations should be rejected");

        assert_eq!(error.code, ApplicationErrorCode::InvalidRequest);
    }

    #[test]
    fn refuses_to_replace_an_existing_output() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let destination = directory.path().join("existing.png");
        fs::write(&destination, b"existing content").expect("test output should be created");

        let error = PendingImageOutput::create_png(Some(&destination))
            .err()
            .expect("existing output should be rejected");

        assert_eq!(error.code, ApplicationErrorCode::OutputAlreadyExists);
        assert_eq!(
            fs::read(destination).expect("existing output should remain readable"),
            b"existing content"
        );
    }

    #[test]
    fn writable_directory_validation_leaves_no_artifact() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");

        validate_writable_output_directory(directory.path())
            .expect("temporary directory should be writable");

        assert_eq!(
            fs::read_dir(directory.path())
                .expect("temporary directory should remain readable")
                .count(),
            0
        );
    }

    #[test]
    fn numbered_publication_uses_the_lowest_gap_without_replacing_files() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let base = directory.path().join("cover-page-0001.png");
        let second = directory.path().join("cover-page-0001 (2).png");
        fs::write(&base, b"base").expect("base output should be written");
        fs::write(&second, b"second").expect("numbered output should be written");
        let mut pending = PendingImageOutput::for_numbered_destination(&base, "png")
            .expect("temporary PNG should be created");
        pending
            .file
            .write_all(b"new")
            .expect("temporary PNG should be writable");

        let published = pending
            .publish()
            .expect("the lowest numbering gap should be used");

        assert_eq!(published, directory.path().join("cover-page-0001 (1).png"));
        assert_eq!(fs::read(base).unwrap(), b"base");
        assert_eq!(fs::read(second).unwrap(), b"second");
        assert_eq!(fs::read(published).unwrap(), b"new");
    }

    #[test]
    fn numbered_publication_detects_case_insensitive_collisions() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        fs::write(directory.path().join("COVER-PAGE-0001.PNG"), b"existing")
            .expect("case-variant output should be written");
        let desired = directory.path().join("cover-page-0001.png");
        let mut pending = PendingImageOutput::for_numbered_destination(&desired, "png")
            .expect("temporary PNG should be created");
        pending
            .file
            .write_all(b"new")
            .expect("temporary PNG should be writable");

        let published = pending
            .publish()
            .expect("case-insensitive collision should be numbered");

        assert_eq!(published, directory.path().join("cover-page-0001 (1).png"));
        assert_eq!(
            fs::read(directory.path().join("COVER-PAGE-0001.PNG")).unwrap(),
            b"existing"
        );
    }

    #[test]
    fn concurrent_numbered_publication_never_clobbers() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let desired = directory.path().join("page.png");
        let pending = [b"first".as_slice(), b"second".as_slice()]
            .into_iter()
            .map(|bytes| {
                let mut output = PendingImageOutput::for_numbered_destination(&desired, "png")
                    .expect("temporary PNG should be created");
                output
                    .file
                    .write_all(bytes)
                    .expect("temporary PNG should be writable");
                output
            })
            .collect::<Vec<_>>();
        let barrier = Arc::new(Barrier::new(pending.len()));
        let handles = pending
            .into_iter()
            .map(|output| {
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    output.publish().expect("publication should retry safely")
                })
            })
            .collect::<Vec<_>>();
        let mut published = handles
            .into_iter()
            .map(|handle| handle.join().expect("publisher should finish"))
            .collect::<Vec<_>>();
        published.sort();

        let mut expected = vec![desired, directory.path().join("page (1).png")];
        expected.sort();
        assert_eq!(published, expected);
        let mut contents = published
            .iter()
            .map(|path| fs::read(path).expect("published PNG should be readable"))
            .collect::<Vec<_>>();
        contents.sort();
        assert_eq!(contents, [b"first".to_vec(), b"second".to_vec()]);
    }

    #[test]
    fn jpg_numbering_preserves_the_selected_extension() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let desired = directory.path().join("cover-page-0001.jpg");
        fs::write(&desired, b"existing").expect("existing JPG should be written");
        let mut pending = PendingImageOutput::for_numbered_destination(&desired, "jpg")
            .expect("temporary image should be created");
        pending
            .file
            .write_all(b"new")
            .expect("temporary image should be writable");

        let published = pending.publish().expect("JPG should be numbered safely");

        assert_eq!(published, directory.path().join("cover-page-0001 (1).jpg"));
        assert_eq!(fs::read(desired).unwrap(), b"existing");
        assert_eq!(fs::read(published).unwrap(), b"new");
    }
}
