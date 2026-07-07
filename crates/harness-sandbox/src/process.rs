use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::debug;

use crate::config::SandboxConfig;
use crate::error::{Result, SandboxError};
use crate::traits::{ExecutionResult, Sandbox};

/// Process-level sandbox: subprocess with timeout, cwd jail, env filtering,
/// and output size limits.
///
/// This is Phase 1 isolation — sufficient for demos and low-risk tools.
/// For untrusted AI-generated code, upgrade to gVisor/Firecracker (see docs/sandbox.md).
pub struct ProcessSandbox {
    config: SandboxConfig,
}

impl ProcessSandbox {
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(SandboxConfig::default())
    }

    fn build_command(&self, command: &str, args: &[&str]) -> Command {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .current_dir(&self.config.workdir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        cmd.env_clear();
        for key in &self.config.allowed_env {
            if let Ok(val) = std::env::var(key) {
                cmd.env(key, val);
            }
        }

        cmd
    }

    async fn read_limited(reader: &mut (impl AsyncReadExt + Unpin), limit: usize) -> String {
        let mut buf = vec![0u8; limit.min(4096)];
        let mut output = Vec::new();

        loop {
            let remaining = limit.saturating_sub(output.len());
            if remaining == 0 {
                break;
            }
            let to_read = buf.len().min(remaining);
            match reader.read(&mut buf[..to_read]).await {
                Ok(0) => break,
                Ok(n) => output.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
        }

        String::from_utf8_lossy(&output).into_owned()
    }
}

#[async_trait]
impl Sandbox for ProcessSandbox {
    async fn exec(&self, command: &str, args: &[&str]) -> Result<ExecutionResult> {
        debug!(command, ?args, workdir = ?self.config.workdir, "process sandbox exec");

        let mut child = self
            .build_command(command, args)
            .spawn()
            .map_err(|e| SandboxError::Execution(format!("spawn: {e}")))?;

        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        let timeout_dur = self.config.timeout;
        let max_bytes = self.config.max_output_bytes;

        let result = timeout(timeout_dur, async {
            let mut stdout = String::new();
            let mut stderr = String::new();

            if let Some(mut out) = stdout_handle {
                stdout = Self::read_limited(&mut out, max_bytes).await;
            }
            if let Some(mut err) = stderr_handle {
                stderr = Self::read_limited(&mut err, max_bytes).await;
            }

            let status = child
                .wait()
                .await
                .map_err(|e| SandboxError::Execution(format!("wait: {e}")))?;

            Ok(ExecutionResult {
                stdout,
                stderr,
                exit_code: status.code(),
                timed_out: false,
            })
        })
        .await;

        match result {
            Ok(inner) => inner,
            Err(_) => {
                let _ = child.kill().await;
                Ok(ExecutionResult {
                    stdout: String::new(),
                    stderr: "execution timed out".into(),
                    exit_code: None,
                    timed_out: true,
                })
            }
        }
    }

    fn name(&self) -> &'static str {
        "process"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runs_echo() {
        let sandbox = ProcessSandbox::with_defaults();
        let result = sandbox.exec("echo", &["hello"]).await.unwrap();
        assert!(result.stdout.contains("hello"));
        assert_eq!(result.exit_code, Some(0));
    }

    #[tokio::test]
    async fn times_out() {
        let sandbox = ProcessSandbox::new(SandboxConfig::default().with_timeout(1));
        let result = sandbox
            .exec("sleep", &["10"])
            .await
            .unwrap();
        assert!(result.timed_out);
    }
}
