use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::{Builder, NamedTempFile};

use super::png_output::validate_output_directory;
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

pub(crate) struct PendingPdfOutput {
    pub(crate) file: NamedTempFile,
    destination_directory: PathBuf,
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

        let mut occupied_names = occupied_names(&self.destination_directory)?;
        let mut file = self.file;
        for number in 0..=u32::MAX {
            let file_name = numbered_pdf_name(stem, number);
            if !occupied_names.insert(collision_key(&file_name)) {
                continue;
            }
            let destination = self.destination_directory.join(file_name);
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
            "A safe output PDF name could not be allocated",
        ))
    }
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

fn numbered_pdf_name(stem: &OsStr, number: u32) -> OsString {
    let mut name = stem.to_os_string();
    if number > 0 {
        name.push(format!(" ({number})"));
    }
    name.push(".pdf");
    name
}

fn collision_key(file_name: &OsStr) -> String {
    file_name.to_string_lossy().to_lowercase()
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
mod tests {
    use std::fs;
    use std::io::Write;

    use super::*;

    #[test]
    fn publishing_uses_the_lowest_available_name_without_replacing_files() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let base = directory.path().join("scan.pdf");
        let second = directory.path().join("scan (2).pdf");
        fs::write(&base, b"base").expect("base fixture should be written");
        fs::write(&second, b"second").expect("second fixture should be written");

        let mut pending = PendingPdfOutput::in_directory(directory.path())
            .expect("temporary output should be created");
        pending
            .file
            .write_all(b"new")
            .expect("temporary PDF should be writable");
        let published = pending
            .publish_with_stem(OsStr::new("scan"))
            .expect("the first numbering gap should be used");

        assert_eq!(published, directory.path().join("scan (1).pdf"));
        assert_eq!(fs::read(base).expect("base remains"), b"base");
        assert_eq!(fs::read(second).expect("second remains"), b"second");
        assert_eq!(fs::read(published).expect("new PDF is readable"), b"new");
    }

    #[test]
    fn filename_allocation_is_case_insensitive() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        fs::write(directory.path().join("SCAN.PDF"), b"existing")
            .expect("case variant should be written");

        let mut pending = PendingPdfOutput::in_directory(directory.path())
            .expect("temporary output should be created");
        pending
            .file
            .write_all(b"new")
            .expect("temporary PDF should be writable");
        let published = pending
            .publish_with_stem(OsStr::new("scan"))
            .expect("case-insensitive collision should be numbered");

        assert_eq!(published, directory.path().join("scan (1).pdf"));
        assert_eq!(
            fs::read(directory.path().join("SCAN.PDF")).unwrap(),
            b"existing"
        );
    }
}
