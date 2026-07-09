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
///
/// Inspired by Codex: shell sandbox applies to shell tool; MCP tools enforce their own guardrails.
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    /// Default level for trusted tasks.
    pub default_level: IsolationLevel,
    /// Use Wasm only for explicit wasm bytecode execution (`task_type=wasm`).
    pub wasm_execution: IsolationLevel,
    /// Use MicroVM for untrusted / AI-generated shell commands.
    pub untrusted: IsolationLevel,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            default_level: IsolationLevel::Process,
            wasm_execution: IsolationLevel::Wasm,
            untrusted: IsolationLevel::MicroVm,
        }
    }
}

impl SandboxPolicy {
    /// Select isolation level based on task type hint.
    pub fn level_for(&self, task_type: &str) -> IsolationLevel {
        match task_type {
            "wasm" => self.wasm_execution,
            "untrusted" | "shell" | "agent-generated" => self.untrusted,
            // trusted, code, script — shell commands at process level
            _ => self.default_level,
        }
    }
}
