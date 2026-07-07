//! Execution trace recording for debugging, evaluation, and model co-evolution.

mod event;
mod tracer;

pub use event::TraceEvent;
pub use tracer::Tracer;
