use std::path::PathBuf;
use std::time::Duration;

/// Resource limits for sandboxed execution.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Maximum execution time.
    pub timeout: Duration,
    /// Working directory (jailed).
    pub workdir: PathBuf,
    /// Maximum stdout/stderr capture size in bytes.
    pub max_output_bytes: usize,
    /// Allowed environment variable keys (empty = inherit none).
    pub allowed_env: Vec<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            workdir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
            max_output_bytes: 64 * 1024,
            allowed_env: vec!["PATH".into(), "HOME".into(), "LANG".into()],
        }
    }
}

impl SandboxConfig {
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout = Duration::from_secs(secs);
        self
    }

    pub fn with_workdir(mut self, path: impl Into<PathBuf>) -> Self {
        self.workdir = path.into();
        self
    }
}
