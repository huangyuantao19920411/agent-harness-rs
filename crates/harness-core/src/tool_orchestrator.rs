use std::sync::Arc;

use harness_tools::ToolRegistry;
use harness_trace::{TraceEvent, Tracer};
use tracing::{info, warn};

use crate::guardian_config::{ApprovalResult, GuardianConfig};
use crate::policy::{ExecPolicy, PolicyDecision};
use crate::provider::ToolCallRef;
use crate::{HarnessError, Result};

/// Approval handler invoked when policy returns NeedsApproval (Codex Guardian-style gate).
#[async_trait::async_trait]
pub trait ApprovalHandler: Send + Sync {
    async fn review(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
        policy_reason: &str,
    ) -> ApprovalResult;
}

/// Auto-approve all pending requests (demo / CI mode).
pub struct AutoApprove;

#[async_trait::async_trait]
impl ApprovalHandler for AutoApprove {
    async fn review(
        &self,
        _tool: &str,
        _args: &serde_json::Value,
        _reason: &str,
    ) -> ApprovalResult {
        ApprovalResult::approved("auto", "auto-approved")
    }
}

/// Deny all pending approval requests.
pub struct DenyAll;

#[async_trait::async_trait]
impl ApprovalHandler for DenyAll {
    async fn review(
        &self,
        _tool: &str,
        _args: &serde_json::Value,
        reason: &str,
    ) -> ApprovalResult {
        ApprovalResult::denied("deny-all", reason)
    }
}

/// Codex-inspired pipeline: policy check → guardian/approval → tool dispatch.
pub struct ToolOrchestrator {
    tools: ToolRegistry,
    policy: ExecPolicy,
    guardian: GuardianConfig,
    approval: Arc<dyn ApprovalHandler>,
    tracer: Option<Tracer>,
}

impl ToolOrchestrator {
    pub fn new(tools: ToolRegistry, policy: ExecPolicy, guardian: GuardianConfig) -> Self {
        Self {
            tools,
            policy,
            guardian,
            approval: Arc::new(AutoApprove),
            tracer: None,
        }
    }

    pub fn with_approval(mut self, handler: Arc<dyn ApprovalHandler>) -> Self {
        self.approval = handler;
        self
    }

    pub fn with_tracer(mut self, tracer: Tracer) -> Self {
        self.tracer = Some(tracer);
        self
    }

    pub fn policy(&self) -> &ExecPolicy {
        &self.policy
    }

    pub fn schemas(&self) -> Vec<harness_tools::ToolSchema> {
        self.tools.schemas()
    }

    pub async fn execute(&self, call: &ToolCallRef, iteration: u32) -> Result<String> {
        let decision = self.policy.evaluate_tool(&call.name, &call.arguments);

        match decision {
            PolicyDecision::Allow if self.needs_guardian_review(&call.name, true) => {
                self.run_approval_gate(call, iteration, "allowlisted command under strict review")
                    .await
            }
            PolicyDecision::Allow => {
                info!(tool = %call.name, "policy: allow");
                self.dispatch(call).await
            }
            PolicyDecision::Deny(reason) => {
                warn!(tool = %call.name, %reason, "policy: deny");
                Err(HarnessError::PolicyDenied(reason))
            }
            PolicyDecision::NeedsApproval(reason) => {
                self.run_approval_gate(call, iteration, &reason).await
            }
        }
    }

    fn needs_guardian_review(&self, _tool_name: &str, was_allowlisted: bool) -> bool {
        if !self.guardian.enabled {
            return false;
        }
        if was_allowlisted {
            return self.guardian.review_allowlisted;
        }
        self.guardian.review_unknown_tools
    }

    async fn run_approval_gate(
        &self,
        call: &ToolCallRef,
        iteration: u32,
        policy_reason: &str,
    ) -> Result<String> {
        info!(tool = %call.name, %policy_reason, "policy: awaiting approval");

        let result = self
            .approval
            .review(&call.name, &call.arguments, policy_reason)
            .await;

        if let Some(tracer) = &self.tracer {
            tracer
                .record(TraceEvent::ToolApprovalReview {
                    iteration,
                    name: call.name.clone(),
                    approved: result.approved,
                    reviewer: result.reviewer.clone(),
                    reason: result.reason.clone(),
                })
                .await;
        }

        if result.approved {
            self.dispatch(call).await
        } else {
            Err(HarnessError::PolicyDenied(format!(
                "approval rejected by {}: {}",
                result.reviewer, result.reason
            )))
        }
    }

    async fn dispatch(&self, call: &ToolCallRef) -> Result<String> {
        self.tools
            .execute(&call.name, &call.arguments)
            .await
            .map_err(HarnessError::Tool)
    }
}
