//! Sandboxed execution for untrusted Agent-generated code.
//!
//! Provides a phased isolation strategy — see `docs/sandbox.md` for the
//! Firecracker vs alternatives analysis.

mod config;
mod error;
mod policy;
mod process;
mod scheduler;
mod traits;
mod wasm;

pub use config::SandboxConfig;
pub use error::{Result, SandboxError};
pub use policy::{IsolationLevel, SandboxPolicy};
pub use process::ProcessSandbox;
pub use scheduler::{render_job_manifest, SandboxScheduler};
pub use traits::{ExecutionResult, Sandbox};
pub use wasm::WasmSandbox;
