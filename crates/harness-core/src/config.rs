use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessConfig {
    pub max_iterations: u32,
    pub max_context_tokens: usize,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            max_context_tokens: 8192,
        }
    }
}
