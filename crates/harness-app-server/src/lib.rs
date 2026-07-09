//! JSON-RPC App Server for Agent Harness (Codex App Server inspired).
//!
//! Exposes the harness core over stdio JSON-RPC 2.0 for IDE/CLI clients.

mod protocol;
mod server;

pub use protocol::{JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
pub use server::AppServer;
