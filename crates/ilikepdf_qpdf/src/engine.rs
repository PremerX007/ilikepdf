use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use ilikepdf_core::{
    StructuralPdfEngine, StructuralPdfEngineFamily, StructuralPdfEngineInfo, StructuralPdfError,
    StructuralPdfOperationResult, StructuralPdfValidation, StructuralPdfVersion,
};

use crate::process::{QpdfProcessError, QpdfProcessOutput, QpdfProcessRunner, QpdfRunner};
use crate::runtime::QpdfRuntimeResolver;

pub struct QpdfCliEngine {
    resolver: QpdfRuntimeResolver,
    runner: Arc<dyn QpdfRunner>,
    compatibility: OnceLock<Result<StructuralPdfEngineInfo, StructuralPdfError>>,
}

impl QpdfCliEngine {
    pub fn bundled() -> Self {
        Self::with_resolver(QpdfRuntimeResolver::bundled())
    }

    pub fn from_runtime_root(runtime_root: impl Into<PathBuf>) -> Self {
        Self::with_resolver(QpdfRuntimeResolver::from_root(runtime_root))
    }

    pub fn from_application_executable(executable: impl Into<PathBuf>) -> Self {
        Self::with_resolver(QpdfRuntimeResolver::from_application_executable(executable))
    }

    fn with_resolver(resolver: QpdfRuntimeResolver) -> Self {
        Self {
            resolver,
            runner: Arc::new(QpdfProcessRunner),
            compatibility: OnceLock::new(),
        }
    }

    #[cfg(test)]
    fn with_runner(resolver: QpdfRuntimeResolver, runner: Arc<dyn QpdfRunner>) -> Self {
        Self {
            resolver,
            runner,
            compatibility: OnceLock::new(),
        }
    }

    fn run(&self, arguments: Vec<OsString>) -> Result<QpdfProcessOutput, StructuralPdfError> {
        let runtime = self.resolver.resolve()?;
        self.runner
            .run(&runtime.executable, &arguments, None)
            .map_err(map_process_error)
    }

    fn probe_uncached(&self) -> Result<StructuralPdfEngineInfo, StructuralPdfError> {
        let runtime = self.resolver.resolve()?;
        let output = self
            .runner
            .run(&runtime.executable, &[OsString::from("--version")], None)
            .map_err(map_process_error)?;
        if output.exit_code != Some(0) {
            return Err(StructuralPdfError::RuntimeIncompatible);
        }
        let version = parse_version(&output.stdout.as_lossy_text())?;
        if version != runtime.expected_version {
            return Err(StructuralPdfError::RuntimeIncompatible);
        }

        Ok(StructuralPdfEngineInfo {
            family: StructuralPdfEngineFamily::Qpdf,
            version,
        })
    }
}

impl StructuralPdfEngine for QpdfCliEngine {
    fn probe(&self) -> Result<StructuralPdfEngineInfo, StructuralPdfError> {
        *self.compatibility.get_or_init(|| self.probe_uncached())
    }

    fn validate(&self, source_path: &Path) -> Result<StructuralPdfValidation, StructuralPdfError> {
        let source_path = validated_source(source_path)?;
        self.probe()?;
        let output = self.run(vec![
            OsString::from("--check"),
            source_path.argument.as_os_str().to_owned(),
        ])?;
        match output.exit_code {
            Some(0) => Ok(StructuralPdfValidation {
                has_warnings: false,
            }),
            Some(3) => Ok(StructuralPdfValidation { has_warnings: true }),
            Some(2) => Err(StructuralPdfError::InvalidDocument),
            _ => Err(StructuralPdfError::OperationFailed),
        }
    }

    fn rewrite(
        &self,
        source_path: &Path,
        working_output_path: &Path,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        let source_path = validated_source(source_path)?;
        let working_output_path = validated_working_output(&source_path, working_output_path)?;
        self.probe()?;
        let output = self.run(vec![
            source_path.argument.as_os_str().to_owned(),
            working_output_path.as_os_str().to_owned(),
        ])?;
        let has_warnings = match output.exit_code {
            Some(0) => false,
            Some(3) => true,
            _ => return Err(StructuralPdfError::OperationFailed),
        };
        match fs::metadata(working_output_path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                Ok(StructuralPdfOperationResult { has_warnings })
            }
            _ => Err(StructuralPdfError::OutputWriteFailed),
        }
    }
}

fn parse_version(output: &str) -> Result<StructuralPdfVersion, StructuralPdfError> {
    let version = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("qpdf version "))
        .ok_or(StructuralPdfError::RuntimeIncompatible)?;
    let mut parts = version.split('.');
    let major = parse_version_component(parts.next())?;
    let minor = parse_version_component(parts.next())?;
    let patch = parse_version_component(parts.next())?;
    if parts.next().is_some() {
        return Err(StructuralPdfError::RuntimeIncompatible);
    }
    Ok(StructuralPdfVersion::new(major, minor, patch))
}

fn parse_version_component(value: Option<&str>) -> Result<u16, StructuralPdfError> {
    value
        .ok_or(StructuralPdfError::RuntimeIncompatible)?
        .parse()
        .map_err(|_| StructuralPdfError::RuntimeIncompatible)
}

struct ValidatedSource {
    argument: PathBuf,
    identity: PathBuf,
}

fn validated_source(source_path: &Path) -> Result<ValidatedSource, StructuralPdfError> {
    match fs::metadata(source_path) {
        Ok(metadata) if metadata.is_file() => Ok(ValidatedSource {
            argument: std::path::absolute(source_path)
                .map_err(|_| StructuralPdfError::SourceUnreadable)?,
            identity: fs::canonicalize(source_path)
                .map_err(|_| StructuralPdfError::SourceUnreadable)?,
        }),
        Ok(_) => Err(StructuralPdfError::SourceNotFile),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(StructuralPdfError::SourceNotFound)
        }
        Err(_) => Err(StructuralPdfError::SourceUnreadable),
    }
}

fn validated_working_output(
    source: &ValidatedSource,
    working_output_path: &Path,
) -> Result<PathBuf, StructuralPdfError> {
    let parent = working_output_path
        .parent()
        .filter(|directory| !directory.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let canonical_parent =
        fs::canonicalize(parent).map_err(|_| StructuralPdfError::OutputWriteFailed)?;
    let file_name = working_output_path
        .file_name()
        .ok_or(StructuralPdfError::OutputWriteFailed)?;
    let output_identity = if working_output_path.exists() {
        fs::canonicalize(working_output_path).map_err(|_| StructuralPdfError::OutputWriteFailed)?
    } else {
        canonical_parent.join(file_name)
    };
    if source.identity == output_identity {
        return Err(StructuralPdfError::OutputWriteFailed);
    }
    if working_output_path.exists() && !working_output_path.is_file() {
        return Err(StructuralPdfError::OutputWriteFailed);
    }
    std::path::absolute(working_output_path).map_err(|_| StructuralPdfError::OutputWriteFailed)
}

fn map_process_error(error: QpdfProcessError) -> StructuralPdfError {
    match error {
        QpdfProcessError::Launch => StructuralPdfError::RuntimeLaunchFailed,
        QpdfProcessError::Stdin | QpdfProcessError::Wait | QpdfProcessError::Capture => {
            StructuralPdfError::OperationFailed
        }
    }
}

#[cfg(test)]
mod tests;
