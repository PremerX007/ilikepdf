use std::path::PathBuf;

use ilikepdf_core::StructuralPdfVersion;

const MANIFEST_SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../third_party/qpdf/runtime-manifest.txt"
));

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeFile {
    pub(crate) relative_path: PathBuf,
    pub(crate) sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeManifest {
    pub(crate) version: StructuralPdfVersion,
    pub(crate) executable: PathBuf,
    pub(crate) runtime_files: Vec<RuntimeFile>,
}

pub(crate) fn runtime_manifest() -> Result<RuntimeManifest, ManifestError> {
    let mut version = None;
    let mut executable = None;
    let mut runtime_files = Vec::new();

    for raw_line in MANIFEST_SOURCE.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once('=').ok_or(ManifestError)?;
        match key {
            "version" => version = Some(parse_version(value)?),
            "executable" => executable = Some(valid_relative_path(value)?),
            "runtime_file" => {
                let (path, sha256) = value.split_once('|').ok_or(ManifestError)?;
                if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err(ManifestError);
                }
                runtime_files.push(RuntimeFile {
                    relative_path: valid_relative_path(path)?,
                    sha256: sha256.to_ascii_lowercase(),
                });
            }
            _ => {}
        }
    }

    let version = version.ok_or(ManifestError)?;
    let executable = executable.ok_or(ManifestError)?;
    if runtime_files.is_empty()
        || !runtime_files
            .iter()
            .any(|file| file.relative_path == executable)
    {
        return Err(ManifestError);
    }

    Ok(RuntimeManifest {
        version,
        executable,
        runtime_files,
    })
}

fn parse_version(value: &str) -> Result<StructuralPdfVersion, ManifestError> {
    let mut parts = value.split('.');
    let major = parts
        .next()
        .ok_or(ManifestError)?
        .parse()
        .map_err(|_| ManifestError)?;
    let minor = parts
        .next()
        .ok_or(ManifestError)?
        .parse()
        .map_err(|_| ManifestError)?;
    let patch = parts
        .next()
        .ok_or(ManifestError)?
        .parse()
        .map_err(|_| ManifestError)?;
    if parts.next().is_some() {
        return Err(ManifestError);
    }
    Ok(StructuralPdfVersion::new(major, minor, patch))
}

fn valid_relative_path(value: &str) -> Result<PathBuf, ManifestError> {
    let path = PathBuf::from(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        Err(ManifestError)
    } else {
        Ok(path)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ManifestError;

#[cfg(test)]
mod tests;
