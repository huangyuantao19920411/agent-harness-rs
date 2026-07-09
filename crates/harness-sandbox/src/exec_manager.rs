//! Unified exec process manager — long-lived shell workers (Codex-inspired).
//!
//! - `managed`: reuse pipe-based shell workers across commands
//! - `pty`: run each command in a fresh pseudo-terminal (TTY-aware)

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::config::SandboxConfig;
use crate::error::{Result, SandboxError};
use crate::pty::PtyShell;
use crate::traits::ExecutionResult;

const MARKER_PREFIX: &str = "__HARNESS_EXEC_END_";
const MARKER_SUFFIX: &str = "__";
const DEFAULT_MAX_PROCESSES: usize = 4;

fn make_marker() -> String {
    format!(
        "{MARKER_PREFIX}{}{MARKER_SUFFIX}",
        &uuid::Uuid::new_v4().to_string()[..8]
    )
}

/// Exec backend: one-shot subprocess vs long-lived managed shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecMode {
    /// Spawn a fresh process per command (default).
    OneShot,
    /// Reuse long-lived shell workers via pipes.
    Managed,
    /// Run each command in a pseudo-terminal (TTY-aware).
    Pty,
}

impl ExecMode {
    pub fn from_env() -> Self {
        match std::env::var("SANDBOX_EXEC_MODE")
            .unwrap_or_else(|_| "oneshot".into())
            .to_lowercase()
            .as_str()
        {
            "managed" | "worker" | "unified" => Self::Managed,
            "pty" | "pseudo" | "terminal" => Self::Pty,
            _ => Self::OneShot,
        }
    }

    pub fn uses_exec_manager(self) -> bool {
        matches!(self, Self::Managed | Self::Pty)
    }
}

/// Request to run a command through the unified exec manager.
#[derive(Debug, Clone)]
pub struct ExecCommandRequest {
    /// Reuse an existing worker, or spawn a new one when `None` (managed mode).
    pub process_id: Option<i32>,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

/// Manages a pool of long-lived shell workers (managed mode) and PTY one-shots.
pub struct ExecProcessManager {
    config: SandboxConfig,
    mode: ExecMode,
    processes: Mutex<HashMap<i32, PipeShell>>,
    next_id: AtomicI32,
    max_processes: usize,
}

struct PipeShell {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    cwd: PathBuf,
}

impl ExecProcessManager {
    pub fn new(config: SandboxConfig, mode: ExecMode) -> Self {
        let max = std::env::var("SANDBOX_EXEC_MAX_PROCESSES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_PROCESSES);

        Self {
            config,
            mode,
            processes: Mutex::new(HashMap::new()),
            next_id: AtomicI32::new(1),
            max_processes: max,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(SandboxConfig::default(), ExecMode::from_env())
    }

    pub fn mode(&self) -> ExecMode {
        self.mode
    }

    pub async fn spawn(&self, cwd: Option<PathBuf>) -> Result<i32> {
        if self.mode != ExecMode::Managed {
            return Err(SandboxError::NotAvailable(
                "spawn() requires SANDBOX_EXEC_MODE=managed".into(),
            ));
        }

        let cwd = cwd.unwrap_or_else(|| self.config.workdir.clone());
        let shell = self.start_pipe_shell(&cwd).await?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        let mut guard = self.processes.lock().await;
        if guard.len() >= self.max_processes {
            if let Some(oldest) = guard.keys().copied().min() {
                if let Some(mut old) = guard.remove(&oldest) {
                    let _ = old._child.kill().await;
                    debug!(oldest, "evicted oldest managed shell");
                }
            }
        }
        guard.insert(id, shell);
        info!(id, ?cwd, "spawned managed shell worker");
        Ok(id)
    }

    pub async fn exec(&self, request: ExecCommandRequest) -> Result<(i32, ExecutionResult)> {
        if self.mode == ExecMode::Pty {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let result = self.exec_pty_once(&request).await?;
            return Ok((id, result));
        }

        let process_id = match request.process_id {
            Some(id) => id,
            None => self.spawn(request.cwd.clone()).await?,
        };

        let result = self
            .exec_on(process_id, &request.command, &request.args, request.cwd)
            .await;

        match result {
            Ok(exec) => Ok((process_id, exec)),
            Err(e) => {
                warn!(process_id, %e, "managed shell failed, removing worker");
                self.processes.lock().await.remove(&process_id);
                Err(e)
            }
        }
    }

    pub async fn write_stdin(&self, process_id: i32, data: &str) -> Result<()> {
        let mut guard = self.processes.lock().await;
        let shell = guard
            .get_mut(&process_id)
            .ok_or_else(|| SandboxError::Execution(format!("unknown process_id {process_id}")))?;

        shell
            .stdin
            .write_all(data.as_bytes())
            .await
            .map_err(|e| SandboxError::Execution(format!("write stdin: {e}")))?;
        shell
            .stdin
            .flush()
            .await
            .map_err(|e| SandboxError::Execution(format!("flush stdin: {e}")))?;
        Ok(())
    }

    pub async fn kill(&self, process_id: i32) -> Result<()> {
        let mut guard = self.processes.lock().await;
        if let Some(mut shell) = guard.remove(&process_id) {
            let _ = shell._child.kill().await;
        }
        Ok(())
    }

    pub async fn active_count(&self) -> usize {
        self.processes.lock().await.len()
    }

    async fn exec_pty_once(&self, request: &ExecCommandRequest) -> Result<ExecutionResult> {
        let cwd = request
            .cwd
            .clone()
            .unwrap_or_else(|| self.config.workdir.clone());
        let script = build_exec_script(&request.command, &request.args, request.cwd.as_deref());
        let marker = make_marker();
        let shell_script = format!("{script}; __EC=$?; printf '%s:%s\\n' '{marker}' \"$__EC\"");
        let max_bytes = self.config.max_output_bytes;
        let allowed_env = self.config.allowed_env.clone();
        let deadline = self.config.timeout;

        let run = tokio::task::spawn_blocking(move || {
            let shell = PtyShell::spawn(&cwd, &allowed_env)?;
            shell.run_script(&shell_script, &marker, max_bytes)
        });

        match timeout(deadline, run).await {
            Ok(Ok(Ok(result))) => Ok(result),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(e)) => Err(SandboxError::Execution(format!("pty task: {e}"))),
            Err(_) => Ok(ExecutionResult {
                stdout: String::new(),
                stderr: "execution timed out".into(),
                exit_code: None,
                timed_out: true,
            }),
        }
    }

    async fn exec_on(
        &self,
        process_id: i32,
        command: &str,
        args: &[String],
        cwd: Option<PathBuf>,
    ) -> Result<ExecutionResult> {
        let script = build_exec_script(command, args, cwd.as_deref());
        let marker = make_marker();
        let payload = format!("{script}\n__EC=$?; echo \"{marker}:$__EC\"\n");

        {
            let mut guard = self.processes.lock().await;
            let shell = guard.get_mut(&process_id).ok_or_else(|| {
                SandboxError::Execution(format!("unknown process_id {process_id}"))
            })?;

            if let Some(ref new_cwd) = cwd {
                shell.cwd = new_cwd.clone();
            }
            shell
                .stdin
                .write_all(payload.as_bytes())
                .await
                .map_err(|e| SandboxError::Execution(format!("write command: {e}")))?;
            shell
                .stdin
                .flush()
                .await
                .map_err(|e| SandboxError::Execution(format!("flush command: {e}")))?;
        }

        self.read_pipe_until_marker(process_id, &marker).await
    }

    async fn read_pipe_until_marker(
        &self,
        process_id: i32,
        marker: &str,
    ) -> Result<ExecutionResult> {
        let deadline = self.config.timeout;
        let max_bytes = self.config.max_output_bytes;
        let marker = marker.to_string();

        let read_fut = async {
            let mut guard = self.processes.lock().await;
            let shell = guard.get_mut(&process_id).ok_or_else(|| {
                SandboxError::Execution(format!("unknown process_id {process_id}"))
            })?;
            read_pipe_until_marker(shell, &marker, max_bytes).await
        };

        match timeout(deadline, read_fut).await {
            Ok(inner) => inner,
            Err(_) => {
                let _ = self.kill(process_id).await;
                Ok(ExecutionResult {
                    stdout: String::new(),
                    stderr: "execution timed out".into(),
                    exit_code: None,
                    timed_out: true,
                })
            }
        }
    }

    async fn start_pipe_shell(&self, cwd: &PathBuf) -> Result<PipeShell> {
        let mut cmd = Command::new("sh");
        cmd.arg("-i")
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env_clear();

        for key in &self.config.allowed_env {
            if let Ok(val) = std::env::var(key) {
                cmd.env(key, val);
            }
        }
        cmd.env("PS1", "").env("PS2", "");

        let mut child = cmd
            .spawn()
            .map_err(|e| SandboxError::Execution(format!("spawn managed shell: {e}")))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| SandboxError::Execution("shell stdin unavailable".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SandboxError::Execution("shell stdout unavailable".into()))?;

        Ok(PipeShell {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
            cwd: cwd.clone(),
        })
    }
}

async fn read_pipe_until_marker(
    shell: &mut PipeShell,
    marker: &str,
    max_bytes: usize,
) -> Result<ExecutionResult> {
    let mut lines = Vec::new();
    let exit_code;
    loop {
        let mut line = String::new();
        let n = shell
            .stdout
            .read_line(&mut line)
            .await
            .map_err(|e| SandboxError::Execution(format!("read stdout: {e}")))?;
        if n == 0 {
            return Err(SandboxError::Execution(
                "managed shell closed stdout".into(),
            ));
        }
        if let Some(rest) = line.strip_prefix(marker) {
            let code_str = rest.trim_start_matches(':').trim();
            exit_code = code_str.parse().unwrap_or(1);
            break;
        }
        if lines.iter().map(|l: &String| l.len()).sum::<usize>() + line.len() <= max_bytes {
            lines.push(line);
        }
    }
    Ok(ExecutionResult {
        stdout: lines.concat(),
        stderr: String::new(),
        exit_code: Some(exit_code),
        timed_out: false,
    })
}

fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".into();
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-:".contains(c))
    {
        return s.into();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn build_exec_script(command: &str, args: &[String], cwd: Option<&std::path::Path>) -> String {
    let mut parts = vec![shell_quote(command)];
    parts.extend(args.iter().map(|a| shell_quote(a)));
    let cmdline = parts.join(" ");

    match cwd {
        Some(dir) => format!("cd {} && {}", shell_quote(&dir.to_string_lossy()), cmdline),
        None => cmdline,
    }
}

pub type SharedExecProcessManager = Arc<ExecProcessManager>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reuses_worker_for_multiple_commands() {
        let mgr = ExecProcessManager::new(SandboxConfig::default(), ExecMode::Managed);
        let (id1, r1) = mgr
            .exec(ExecCommandRequest {
                process_id: None,
                command: "echo".into(),
                args: vec!["first".into()],
                cwd: None,
            })
            .await
            .unwrap();
        let (id2, r2) = mgr
            .exec(ExecCommandRequest {
                process_id: Some(id1),
                command: "echo".into(),
                args: vec!["second".into()],
                cwd: None,
            })
            .await
            .unwrap();

        assert_eq!(id1, id2);
        assert!(r1.stdout.contains("first"));
        assert!(r2.stdout.contains("second"));
        assert_eq!(r1.exit_code, Some(0));
        assert_eq!(r2.exit_code, Some(0));
    }

    #[tokio::test]
    async fn pty_worker_runs_echo() {
        let mgr = ExecProcessManager::new(SandboxConfig::default(), ExecMode::Pty);
        let (_id, result) = mgr
            .exec(ExecCommandRequest {
                process_id: None,
                command: "echo".into(),
                args: vec!["pty-ok".into()],
                cwd: None,
            })
            .await
            .unwrap();
        assert!(result.stdout.contains("pty-ok"));
        assert_eq!(result.exit_code, Some(0));
    }

    #[tokio::test]
    async fn exec_mode_from_env() {
        std::env::set_var("SANDBOX_EXEC_MODE", "managed");
        assert_eq!(ExecMode::from_env(), ExecMode::Managed);
        std::env::set_var("SANDBOX_EXEC_MODE", "pty");
        assert_eq!(ExecMode::from_env(), ExecMode::Pty);
        std::env::remove_var("SANDBOX_EXEC_MODE");
        assert_eq!(ExecMode::from_env(), ExecMode::OneShot);
    }

    #[test]
    fn shell_quote_special_chars() {
        assert_eq!(shell_quote("hello"), "hello");
        assert_eq!(shell_quote("a b"), "'a b'");
    }
}
