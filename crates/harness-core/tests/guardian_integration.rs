use std::sync::Arc;

use harness_core::{
    parse_guardian_response, ApprovalHandler, ApprovalMode, ExecPolicy, GuardianDecision,
    GuardianReviewer, ToolOrchestrator,
};
use harness_models::MockModel;
use harness_tools::{Tool, ToolRegistry, ToolSchema};
use harness_trace::{TraceEvent, Tracer};
use async_trait::async_trait;

struct EchoShellTool;

#[async_trait]
impl Tool for EchoShellTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "sandbox_exec".into(),
            description: "run shell".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, args: &serde_json::Value) -> harness_tools::Result<String> {
        let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("executed: {cmd}"))
    }
}

#[tokio::test]
async fn guardian_denies_destructive_command() {
    let model = Arc::new(MockModel);
    let reviewer = GuardianReviewer::new(Arc::clone(&model), Default::default());

    let result = reviewer
        .review(
            "sandbox_exec",
            &serde_json::json!({"command": "rm", "args": ["-rf", "/"]}),
            "shell command requires approval",
        )
        .await;

    assert!(!result.approved);
    assert!(result.reason.contains("destructive"));
}

#[tokio::test]
async fn guardian_approves_safe_command() {
    let model = Arc::new(MockModel);
    let reviewer = GuardianReviewer::new(Arc::clone(&model), Default::default());

    let result = reviewer
        .review(
            "sandbox_exec",
            &serde_json::json!({"command": "ls", "args": ["-la"]}),
            "shell command requires approval",
        )
        .await;

    assert!(result.approved);
}

#[tokio::test]
async fn orchestrator_records_guardian_trace() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(EchoShellTool));

    let tracer = Tracer::new();
    let model = Arc::new(MockModel);
    let guardian = GuardianReviewer::new(Arc::clone(&model), Default::default());

    let orchestrator = ToolOrchestrator::new(
        registry,
        ExecPolicy {
            mode: ApprovalMode::Prompt,
            ..ExecPolicy::default()
        },
        Default::default(),
    )
    .with_approval(Arc::new(guardian))
    .with_tracer(tracer.clone());

    let call = harness_core::ToolCallRef {
        id: "1".into(),
        name: "sandbox_exec".into(),
        arguments: serde_json::json!({"command": "python", "args": ["-c", "print(1)"]}),
    };

    let out = orchestrator.execute(&call, 1).await.unwrap();
    assert!(out.contains("executed"));

    let events = tracer.events().await;
    assert!(events.iter().any(|e| matches!(
        e,
        TraceEvent::ToolApprovalReview {
            approved: true,
            ..
        }
    )));
}

#[test]
fn parse_guardian_deny_response() {
    let d = parse_guardian_response("DECISION: DENY\nREASON: bad");
    assert_eq!(
        d,
        GuardianDecision::Deny {
            reason: "bad".into()
        }
    );
}
