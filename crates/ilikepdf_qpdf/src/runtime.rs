use std::fs;
use std::path::{Path, PathBuf};

use ilikepdf_core::{StructuralPdfError, StructuralPdfVersion};

use crate::manifest::runtime_manifest;

#[derive(Debug, Clone)]
pub(crate) struct ResolvedQpdfRuntime {
    pub(crate) executable: PathBuf,
    pub(crate) expected_version: StructuralPdfVersion,
}

#[derive(Debug, Clone)]
enum RuntimeLocation {
    CurrentApplication,
    ApplicationExecutable(PathBuf),
    ExplicitRoot(PathBuf),
}

#[derive(Debug, Clone)]
pub(crate) struct QpdfRuntimeResolver {
    location: RuntimeLocation,
}

impl QpdfRuntimeResolver {
    pub(crate) fn bundled() -> Self {
        Self {
            location: RuntimeLocation::CurrentApplication,
        }
    }

    pub(crate) fn from_application_executable(executable: impl Into<PathBuf>) -> Self {
        Self {
            location: RuntimeLocation::ApplicationExecutable(executable.into()),
        }
    }

    pub(crate) fn from_root(root: impl Into<PathBuf>) -> Self {
        Self {
            location: RuntimeLocation::ExplicitRoot(root.into()),
        }
    }

    pub(crate) fn resolve(&self) -> Result<ResolvedQpdfRuntime, StructuralPdfError> {
        let manifest = runtime_manifest().map_err(|_| StructuralPdfError::RuntimeIncompatible)?;
        let root = fs::canonicalize(self.runtime_root()?)
            .map_err(|_| StructuralPdfError::RuntimeUnavailable)?;

        for file in &manifest.runtime_files {
            if !root.join(&file.relative_path).is_file() {
                return Err(StructuralPdfError::RuntimeUnavailable);
            }
        }

        Ok(ResolvedQpdfRuntime {
            executable: root.join(manifest.executable),
            expected_version: manifest.version,
        })
    }

    fn runtime_root(&self) -> Result<PathBuf, StructuralPdfError> {
        match &self.location {
            RuntimeLocation::CurrentApplication => {
                let executable =
                    std::env::current_exe().map_err(|_| StructuralPdfError::RuntimeUnavailable)?;
                application_runtime_root(&executable)
            }
            RuntimeLocation::ApplicationExecutable(executable) => {
                application_runtime_root(executable)
            }
            RuntimeLocation::ExplicitRoot(root) => Ok(root.clone()),
        }
    }
}

fn application_runtime_root(executable: &Path) -> Result<PathBuf, StructuralPdfError> {
    executable
        .parent()
        .filter(|directory| !directory.as_os_str().is_empty())
        .map(|directory| directory.join("runtime").join("qpdf"))
        .ok_or(StructuralPdfError::RuntimeUnavailable)
}

#[cfg(test)]
mod tests;
