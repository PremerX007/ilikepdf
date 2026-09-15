use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::{Builder, TempDir};

use super::image::validate_output_directory;
use super::publication::{collision_key, occupied_names};
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

pub(crate) struct PendingSplitDirectory {
    directory: TempDir,
    destination_directory: PathBuf,
    output_stem: OsString,
}

impl PendingSplitDirectory {
    pub(crate) fn in_directory(
        destination_directory: &Path,
        source_stem: &OsStr,
    ) -> ApplicationResult<Self> {
        validate_output_directory(destination_directory)?;
        if source_stem.is_empty() {
            return Err(ApplicationError::new(
                ApplicationErrorCode::InvalidRequest,
                "The source PDF must have a file name",
            ));
        }
        let directory = Builder::new()
            .prefix(".ilikepdf-split-pdf-")
            .tempdir_in(destination_directory)
            .map_err(|_| temporary_directory_error())?;
        let mut output_stem = source_stem.to_os_string();
        output_stem.push("-split");
        Ok(Self {
            directory,
            destination_directory: destination_directory.to_path_buf(),
            output_stem,
        })
    }

    pub(crate) fn working_path(&self) -> &Path {
        self.directory.path()
    }

    pub(crate) fn publish(self) -> ApplicationResult<PathBuf> {
        let mut occupied =
            occupied_names(&self.destination_directory).map_err(|_| publication_error())?;
        let private_path = self.directory.keep();

        for number in 0..=u32::MAX {
            let candidate_name = numbered_directory_name(&self.output_stem, number);
            if !occupied.insert(collision_key(&candidate_name)) {
                continue;
            }
            let destination = self.destination_directory.join(candidate_name);
            match fs::rename(&private_path, &destination) {
                Ok(()) => return Ok(destination),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::DirectoryNotEmpty
                    ) || (error.kind() == std::io::ErrorKind::PermissionDenied
                        && destination.exists()) =>
                {
                    continue;
                }
                Err(_) => {
                    let _ = fs::remove_dir_all(&private_path);
                    return Err(publication_error());
                }
            }
        }

        let _ = fs::remove_dir_all(&private_path);
        Err(publication_error())
    }
}

fn numbered_directory_name(stem: &OsStr, number: u32) -> OsString {
    let mut name = stem.to_os_string();
    if number > 0 {
        name.push(format!(" ({number})"));
    }
    name
}

fn temporary_directory_error() -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::TemporaryDirectoryFailed,
        "A private split workspace could not be created",
    )
}

fn publication_error() -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::PublicationFailed,
        "The split output folder could not be published safely",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn allocates_initial_suffix_and_lowest_gap_without_touching_existing_folders() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("report-split")).unwrap();
        fs::write(
            directory.path().join("report-split").join("existing.txt"),
            b"keep",
        )
        .unwrap();
        fs::create_dir(directory.path().join("report-split (2)")).unwrap();
        let pending =
            PendingSplitDirectory::in_directory(directory.path(), OsStr::new("report")).unwrap();
        fs::write(pending.working_path().join("part.pdf"), b"part").unwrap();

        let published = pending.publish().unwrap();

        assert_eq!(published, directory.path().join("report-split (1)"));
        assert_eq!(
            fs::read(directory.path().join("report-split").join("existing.txt")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn collision_matching_is_case_insensitive() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("REPORT-SPLIT")).unwrap();
        let pending =
            PendingSplitDirectory::in_directory(directory.path(), OsStr::new("report")).unwrap();
        let published = pending.publish().unwrap();
        assert_eq!(published, directory.path().join("report-split (1)"));
    }

    #[test]
    #[cfg(windows)]
    fn concurrent_directory_publication_never_merges_or_clobbers() {
        let directory = tempfile::tempdir().unwrap();
        let pending = ["first", "second"]
            .into_iter()
            .map(|content| {
                let pending =
                    PendingSplitDirectory::in_directory(directory.path(), OsStr::new("report"))
                        .unwrap();
                fs::write(pending.working_path().join("part.pdf"), content).unwrap();
                pending
            })
            .collect::<Vec<_>>();
        let barrier = Arc::new(Barrier::new(pending.len()));
        let handles = pending
            .into_iter()
            .map(|pending| {
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    pending.publish().unwrap()
                })
            })
            .collect::<Vec<_>>();
        let mut published = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        published.sort();
        let mut expected = vec![
            directory.path().join("report-split"),
            directory.path().join("report-split (1)"),
        ];
        expected.sort();
        assert_eq!(published, expected);
        let mut contents = published
            .iter()
            .map(|path| fs::read_to_string(path.join("part.pdf")).unwrap())
            .collect::<Vec<_>>();
        contents.sort();
        assert_eq!(contents, ["first", "second"]);
    }
}
