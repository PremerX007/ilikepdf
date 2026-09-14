use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;

const DIAGNOSTIC_LIMIT: usize = 32 * 1024;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(crate) trait QpdfRunner: Send + Sync {
    fn run(
        &self,
        executable: &Path,
        arguments: &[OsString],
        stdin_data: Option<&[u8]>,
    ) -> Result<QpdfProcessOutput, QpdfProcessError>;
}

pub(crate) struct QpdfProcessRunner;

impl QpdfRunner for QpdfProcessRunner {
    fn run(
        &self,
        executable: &Path,
        arguments: &[OsString],
        stdin_data: Option<&[u8]>,
    ) -> Result<QpdfProcessOutput, QpdfProcessError> {
        let mut command = Command::new(executable);
        command
            .args(arguments)
            .current_dir(executable.parent().unwrap_or_else(|| Path::new(".")))
            .env_remove("QPDF_EXECUTABLE")
            .env_remove("QPDF_FIX_QDF")
            .env_remove("QPDF_ZLIB_FLATE")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(if stdin_data.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            });

        #[cfg(windows)]
        configure_windows_process(&mut command);

        let mut child = command.spawn().map_err(|_| QpdfProcessError::Launch)?;
        let stdout = child.stdout.take().ok_or_else(|| {
            terminate_and_reap(&mut child);
            QpdfProcessError::Capture
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            terminate_and_reap(&mut child);
            QpdfProcessError::Capture
        })?;
        let stdout_thread = thread::spawn(move || capture_bounded(stdout));
        let stderr_thread = thread::spawn(move || capture_bounded(stderr));

        if let Some(data) = stdin_data {
            let stdin_result = child
                .stdin
                .take()
                .ok_or(QpdfProcessError::Stdin)
                .and_then(|mut stdin| stdin.write_all(data).map_err(|_| QpdfProcessError::Stdin));
            if let Err(error) = stdin_result {
                terminate_and_reap(&mut child);
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
                return Err(error);
            }
        }

        let status = match child.wait() {
            Ok(status) => status,
            Err(_) => {
                terminate_and_reap(&mut child);
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
                return Err(QpdfProcessError::Wait);
            }
        };
        let stdout = stdout_thread
            .join()
            .map_err(|_| QpdfProcessError::Capture)?
            .map_err(|_| QpdfProcessError::Capture)?;
        let stderr = stderr_thread
            .join()
            .map_err(|_| QpdfProcessError::Capture)?
            .map_err(|_| QpdfProcessError::Capture)?;

        Ok(QpdfProcessOutput {
            exit_code: status.code(),
            stdout,
            stderr,
        })
    }
}

#[cfg(windows)]
fn configure_windows_process(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    command.creation_flags(qpdf_windows_creation_flags());
}

#[cfg(windows)]
const fn qpdf_windows_creation_flags() -> u32 {
    CREATE_NO_WINDOW
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundedDiagnostic {
    bytes: Vec<u8>,
    truncated: bool,
}

impl BoundedDiagnostic {
    pub(crate) fn as_lossy_text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }

    #[cfg(test)]
    pub(crate) fn for_test(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.to_vec(),
            truncated: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QpdfProcessOutput {
    pub(crate) exit_code: Option<i32>,
    pub(crate) stdout: BoundedDiagnostic,
    pub(crate) stderr: BoundedDiagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QpdfProcessError {
    Launch,
    Stdin,
    Wait,
    Capture,
}

fn capture_bounded(mut reader: impl Read) -> io::Result<BoundedDiagnostic> {
    let mut captured = Vec::with_capacity(DIAGNOSTIC_LIMIT.min(4096));
    let mut truncated = false;
    let mut buffer = [0_u8; 4096];

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = DIAGNOSTIC_LIMIT.saturating_sub(captured.len());
        let kept = remaining.min(count);
        captured.extend_from_slice(&buffer[..kept]);
        truncated |= kept < count;
    }

    Ok(BoundedDiagnostic {
        bytes: captured,
        truncated,
    })
}

fn terminate_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests;
