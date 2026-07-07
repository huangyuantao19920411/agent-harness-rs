/// Isolation level for sandboxed execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IsolationLevel {
    /// Subprocess with timeout and env filtering.
    Process = 1,
    /// WebAssembly bytecode sandbox.
    Wasm = 2,
    /// Kubernetes Job with gVisor or Firecracker RuntimeClass.
    MicroVm = 3,
}

/// Policy that maps task risk to isolation backend.
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    /// Default level for unknown tasks.
    pub default_level: IsolationLevel,
    /// Use Wasm for code execution tasks.
    pub code_execution: IsolationLevel,
    /// Use MicroVM for untrusted / AI-generated shell commands.
    pub untrusted: IsolationLevel,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            default_level: IsolationLevel::Process,
            code_execution: IsolationLevel::Wasm,
            untrusted: IsolationLevel::MicroVm,
        }
    }
}

impl SandboxPolicy {
    /// Select isolation level based on task type hint.
    pub fn level_for(&self, task_type: &str) -> IsolationLevel {
        match task_type {
            "code" | "wasm" | "script" => self.code_execution,
            "untrusted" | "shell" | "agent-generated" => self.untrusted,
            _ => self.default_level,
        }
    }
}
