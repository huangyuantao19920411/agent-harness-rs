//! Execution trace recording for debugging, evaluation, and model co-evolution.

mod event;
mod persist;
mod tracer;

pub use event::TraceEvent;
pub use persist::{load_trace, replay_summary, TraceWriter};
pub use tracer::Tracer;
