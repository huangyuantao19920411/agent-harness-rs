use std::sync::Arc;
use tokio::sync::RwLock;

use crate::TraceEvent;

#[derive(Clone, Default)]
pub struct Tracer {
    events: Arc<RwLock<Vec<TraceEvent>>>,
}

impl Tracer {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn record(&self, event: TraceEvent) {
        self.events.write().await.push(event);
    }

    pub async fn events(&self) -> Vec<TraceEvent> {
        self.events.read().await.clone()
    }

    pub async fn clear(&self) {
        self.events.write().await.clear();
    }
}
