//! Sandboxed execution for untrusted Agent-generated code.
//!
//! Provides a phased isolation strategy — see `docs/sandbox.md` for the
//! Firecracker vs alternatives analysis.

mod config;
mod error;
mod process;
mod traits;

pub use config::SandboxConfig;
pub use error::{Result, SandboxError};
pub use process::ProcessSandbox;
pub use traits::{ExecutionResult, Sandbox};
