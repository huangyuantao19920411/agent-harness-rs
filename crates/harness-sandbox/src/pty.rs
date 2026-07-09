//! PTY (pseudo-terminal) sessions for interactive shell execution.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tracing::debug;

use crate::error::{Result, SandboxError};
use crate::traits::ExecutionResult;

const DEFAULT_ROWS: u16 = 24;
const DEFAULT_COLS: u16 = 120;

/// Long-lived shell running inside a pseudo-terminal.
pub struct PtyShell {
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

/// Cloneable handle for blocking PTY operations from async contexts.
pub(crate) struct PtyShellHandle {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

impl PtyShell {
    /// Spawn an interactive `sh` in a PTY at `cwd`.
    pub fn spawn(cwd: &Path, allowed_env: &[String]) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: DEFAULT_ROWS,
                cols: DEFAULT_COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| SandboxError::Execution(format!("open pty: {e}")))?;

        let mut cmd = CommandBuilder::new("sh");
        cmd.cwd(cwd);
        cmd.env("TERM", "dumb");
        cmd.env("NO_COLOR", "1");

        for key in allowed_env {
            if let Ok(val) = std::env::var(key) {
                cmd.env(key, val);
            }
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| SandboxError::Execution(format!("spawn pty shell: {e}")))?;
        drop(pair.slave);

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| SandboxError::Execution(format!("pty writer: {e}")))?;

        debug!(?cwd, "spawned PTY shell");

        Ok(Self {
            master: Arc::new(Mutex::new(pair.master)),
            writer: Arc::new(Mutex::new(writer)),
            child: Arc::new(Mutex::new(child)),
        })
    }

    /// Run a one-shot script inside the PTY and capture output until marker line.
    pub fn run_script(&self, script: &str, marker: &str, max_bytes: usize) -> Result<ExecutionResult> {
        let payload = format!("{script}\r");
        self.handle().write_all(payload.as_bytes())?;
        let raw = self.handle().read_until(marker, max_bytes)?;
        self.kill()?;
        let (stdout, exit_code) = split_marker_output(&raw, marker);
        Ok(ExecutionResult {
            stdout,
            stderr: String::new(),
            exit_code: Some(exit_code),
            timed_out: false,
        })
    }

    pub(crate) fn handle(&self) -> PtyShellHandle {
        PtyShellHandle {
            writer: Arc::clone(&self.writer),
            master: Arc::clone(&self.master),
            child: Arc::clone(&self.child),
        }
    }

    pub fn kill(&self) -> Result<()> {
        self.handle().kill()
    }
}

impl PtyShellHandle {
    pub fn write_all(&self, data: &[u8]) -> Result<()> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|e| SandboxError::Execution(format!("pty writer lock: {e}")))?;
        let normalized: Vec<u8> = data
            .iter()
            .flat_map(|&b| {
                if b == b'\n' {
                    vec![b'\r', b'\n']
                } else {
                    vec![b]
                }
            })
            .collect();
        writer
            .write_all(&normalized)
            .map_err(|e| SandboxError::Execution(format!("pty write: {e}")))?;
        writer
            .flush()
            .map_err(|e| SandboxError::Execution(format!("pty flush: {e}")))?;
        Ok(())
    }

    pub fn read_until(&self, marker: &str, max_bytes: usize) -> Result<String> {
        let master = self
            .master
            .lock()
            .map_err(|e| SandboxError::Execution(format!("pty master lock: {e}")))?;
        let mut reader = master
            .try_clone_reader()
            .map_err(|e| SandboxError::Execution(format!("pty reader: {e}")))?;

        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let needle = format!("{marker}:");

        loop {
            if buf.len() >= max_bytes {
                break;
            }
            let n = reader
                .read(&mut chunk)
                .map_err(|e| SandboxError::Execution(format!("pty read: {e}")))?;
            if n == 0 {
                return Err(SandboxError::Execution("pty closed".into()));
            }
            buf.extend_from_slice(&chunk[..n]);
            if String::from_utf8_lossy(&buf).contains(&needle) {
                break;
            }
        }

        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    pub fn kill(&self) -> Result<()> {
        let mut child = self
            .child
            .lock()
            .map_err(|e| SandboxError::Execution(format!("pty child lock: {e}")))?;
        child
            .kill()
            .map_err(|e| SandboxError::Execution(format!("pty kill: {e}")))?;
        Ok(())
    }
}

fn split_marker_output(raw: &str, marker: &str) -> (String, i32) {
    let normalized = raw.replace('\r', "");
    let needle = format!("{marker}:");
    if let Some(idx) = normalized.rfind(&needle) {
        let stdout = normalized[..idx].to_string();
        let rest = normalized[idx + needle.len()..].trim();
        let exit_code = rest
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(1);
        (stdout, exit_code)
    } else {
        (normalized, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn spawns_and_runs_echo() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp"));
        let shell = PtyShell::spawn(&cwd, &["PATH".into()]).unwrap();
        let marker = "__TEST_END__";
        let result = shell
            .run_script(
                &format!("echo pty-hello; printf '%s:%s\\n' '{marker}' \"$?\""),
                marker,
                8192,
            )
            .unwrap();
        assert!(result.stdout.contains("pty-hello"));
        assert_eq!(result.exit_code, Some(0));
    }
}
