use async_trait::async_trait;
use harness_core::{CompletionRequest, CompletionResult, ModelProvider};

use crate::{DeepSeekModel, MockModel};

/// Select model backend from environment or fallback to mock.
pub enum ModelBackend {
    Mock(MockModel),
    DeepSeek(DeepSeekModel),
}

impl ModelBackend {
    pub fn from_env() -> Self {
        DeepSeekModel::from_env()
            .map(Self::DeepSeek)
            .unwrap_or(Self::Mock(MockModel))
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Mock(_) => "mock",
            Self::DeepSeek(_) => "deepseek",
        }
    }
}

#[async_trait]
impl ModelProvider for ModelBackend {
    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<CompletionResult, String> {
        match self {
            Self::Mock(model) => model.complete(request).await,
            Self::DeepSeek(model) => model.complete(request).await,
        }
    }
}

/// Alias for applications that want a single type.
pub type AnyModel = ModelBackend;
