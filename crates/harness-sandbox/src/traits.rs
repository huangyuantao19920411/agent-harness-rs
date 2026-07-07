use async_trait::async_trait;

/// Result of a sandboxed execution.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

/// Trait for sandbox backends (process, wasm, microvm, ...).
#[async_trait]
pub trait Sandbox: Send + Sync {
    /// Execute a shell command in the sandbox.
    async fn exec(&self, command: &str, args: &[&str]) -> crate::Result<ExecutionResult>;

    /// Name of this sandbox backend.
    fn name(&self) -> &'static str;
}
