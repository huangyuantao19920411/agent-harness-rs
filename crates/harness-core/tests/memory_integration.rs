use std::sync::Arc;

use harness_core::{extract_and_store, AgentLoop, AgentRequest, HarnessConfig, Message};
use harness_memory::{MemoryConfig, MemoryStore};
use harness_models::MockModel;
use harness_tools::{Tool, ToolRegistry, ToolSchema};
use harness_trace::{TraceEvent, Tracer};
use async_trait::async_trait;
use tempfile::NamedTempFile;

struct NoopTool;

#[async_trait]
impl Tool for NoopTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "noop".into(),
            description: "noop".into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn execute(&self, _args: &serde_json::Value) -> harness_tools::Result<String> {
        Ok("done".into())
    }
}

#[tokio::test]
async fn memory_pipeline_persists_and_recalls() {
    let tmp = NamedTempFile::new().unwrap();
    let db_path = tmp.path().to_path_buf();

    let memory = MemoryConfig {
        enabled: true,
        db_path: db_path.clone(),
        extract_on_complete: true,
        global_recall: true,
        max_recall: 5,
        ..Default::default()
    };

    let config = HarnessConfig {
        max_iterations: 2,
        guardian: harness_core::GuardianConfig::disabled(),
        exec_policy: harness_core::ExecPolicy {
            mode: harness_core::ApprovalMode::Auto,
            ..Default::default()
        },
        memory: memory.clone(),
        ..Default::default()
    };

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(NoopTool));
    let tracer = Tracer::new();

    let loop1 = AgentLoop::new(MockModel, registry.clone(), tracer.clone(), config.clone());
    loop1
        .run(AgentRequest {
            input: "Tell me about Rust backend".into(),
            system_prompt: Some("test".into()),
            session_id: Some("session-1".into()),
        })
        .await
        .unwrap();

    let store = MemoryStore::open(&db_path).unwrap();
    assert!(store.count().unwrap() >= 1);

    let loop2 = AgentLoop::new(MockModel, registry, tracer.clone(), config);
    loop2
        .run(AgentRequest {
            input: "What do you remember?".into(),
            system_prompt: Some("test".into()),
            session_id: Some("session-2".into()),
        })
        .await
        .unwrap();

    let events = tracer.events().await;
    assert!(events.iter().any(|e| matches!(e, TraceEvent::MemoryPersisted { .. })));
    assert!(events.iter().any(|e| matches!(e, TraceEvent::MemoryRecalled { .. })));
}

#[tokio::test]
async fn extract_and_store_directly() {
    let tmp = NamedTempFile::new().unwrap();
    let store = MemoryStore::open(tmp.path()).unwrap();
    let model = Arc::new(MockModel);

    let messages = vec![
        Message::user("I use Rust"),
        Message::assistant("Great choice"),
    ];

    let config = MemoryConfig {
        enabled: true,
        extract_on_complete: true,
        ..Default::default()
    };

    let count = extract_and_store(&model, &store, "s1", &messages, &config, Some(1))
        .await
        .unwrap();
    assert!(count >= 1);
    assert!(store.count().unwrap() >= 1);
}
