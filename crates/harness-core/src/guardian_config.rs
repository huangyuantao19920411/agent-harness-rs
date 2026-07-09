use serde::{Deserialize, Serialize};

/// Guardian LLM reviewer configuration (Codex Guardian-inspired).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianConfig {
    /// Enable LLM review for tools requiring approval.
    pub enabled: bool,
    /// Also review allowlisted shell commands (strict mode).
    pub review_allowlisted: bool,
    /// Review non-exempt tools that policy would auto-allow.
    pub review_unknown_tools: bool,
}

impl Default for GuardianConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            review_allowlisted: false,
            review_unknown_tools: false,
        }
    }
}

impl GuardianConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }
}

/// Outcome of an approval review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalResult {
    pub approved: bool,
    pub reviewer: String,
    pub reason: String,
}

impl ApprovalResult {
    pub fn approved(reviewer: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            approved: true,
            reviewer: reviewer.into(),
            reason: reason.into(),
        }
    }

    pub fn denied(reviewer: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            approved: false,
            reviewer: reviewer.into(),
            reason: reason.into(),
        }
    }
}
