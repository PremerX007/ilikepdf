use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

pub(super) enum NumberedPublicationError {
    Io(io::Error),
    Exhausted,
}

pub(super) fn publish_numbered(
    mut file: NamedTempFile,
    directory: &Path,
    mut candidate_name: impl FnMut(u32) -> OsString,
) -> Result<PathBuf, NumberedPublicationError> {
    let mut occupied_names = occupied_names(directory).map_err(NumberedPublicationError::Io)?;

    for number in 0..=u32::MAX {
        let file_name = candidate_name(number);
        if !occupied_names.insert(collision_key(&file_name)) {
            continue;
        }
        let destination = directory.join(file_name);
        match file.persist_noclobber(&destination) {
            Ok(_) => return Ok(destination),
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
                file = error.file;
            }
            Err(error) => return Err(NumberedPublicationError::Io(error.error)),
        }
    }

    Err(NumberedPublicationError::Exhausted)
}

pub(crate) fn occupied_names(directory: &Path) -> io::Result<HashSet<String>> {
    fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| collision_key(&entry.file_name())))
        .collect()
}

pub(crate) fn collision_key(file_name: &OsStr) -> String {
    file_name.to_string_lossy().to_lowercase()
}
