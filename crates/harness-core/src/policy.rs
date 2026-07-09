use serde::{Deserialize, Serialize};

/// How tool executions are approved (Codex-style policy gate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    /// Auto-approve tools matching allow rules; deny blocked patterns.
    Auto,
    /// Require explicit approval for non-allowlisted shell commands.
    Prompt,
    /// Deny any tool not explicitly allowlisted.
    DenyUnknown,
}

/// Rule-based exec policy inspired by Codex execpolicy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecPolicy {
    pub mode: ApprovalMode,
    /// Prefixes that are always allowed (e.g. "cargo ", "git status").
    pub allowed_prefixes: Vec<String>,
    /// Substrings that are always denied (e.g. "rm -rf", "curl | sh").
    pub denied_patterns: Vec<String>,
    /// Tool names exempt from shell policy (MCP tools manage their own guardrails).
    pub exempt_tools: Vec<String>,
}

impl Default for ExecPolicy {
    fn default() -> Self {
        Self {
            mode: ApprovalMode::Auto,
            allowed_prefixes: vec![
                "echo ".into(),
                "ls".into(),
                "pwd".into(),
                "cat ".into(),
                "cargo ".into(),
                "git status".into(),
                "git diff".into(),
                "git log".into(),
            ],
            denied_patterns: vec![
                "rm -rf".into(),
                "rm -fr".into(),
                "curl | sh".into(),
                "wget | sh".into(),
                "chmod 777".into(),
                "mkfs".into(),
                ":(){ :|:& };:".into(),
            ],
            exempt_tools: vec!["list_dir".into()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Deny(String),
    NeedsApproval(String),
}

impl ExecPolicy {
    pub fn evaluate_tool(&self, tool_name: &str, arguments: &serde_json::Value) -> PolicyDecision {
        if self.exempt_tools.iter().any(|t| t == tool_name) {
            return PolicyDecision::Allow;
        }

        if tool_name == "sandbox_exec" {
            return self.evaluate_shell(arguments);
        }

        match self.mode {
            ApprovalMode::Auto => PolicyDecision::Allow,
            ApprovalMode::Prompt => PolicyDecision::NeedsApproval(format!(
                "tool '{tool_name}' requires approval"
            )),
            ApprovalMode::DenyUnknown => PolicyDecision::NeedsApproval(format!(
                "tool '{tool_name}' not in allowlist"
            )),
        }
    }

    fn evaluate_shell(&self, args: &serde_json::Value) -> PolicyDecision {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let cmd_args: Vec<&str> = args
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        let full = if cmd_args.is_empty() {
            command.to_string()
        } else {
            format!("{command} {}", cmd_args.join(" "))
        };

        for pattern in &self.denied_patterns {
            if full.contains(pattern) {
                return PolicyDecision::Deny(format!("blocked pattern: {pattern}"));
            }
        }

        if self
            .allowed_prefixes
            .iter()
            .any(|p| full.starts_with(p) || full == p.trim())
        {
            return PolicyDecision::Allow;
        }

        match self.mode {
            ApprovalMode::Auto => PolicyDecision::Allow,
            ApprovalMode::Prompt => PolicyDecision::NeedsApproval(format!(
                "shell command requires approval: {full}"
            )),
            ApprovalMode::DenyUnknown => PolicyDecision::Deny(format!(
                "command not in allowlist: {full}"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denies_rm_rf() {
        let policy = ExecPolicy::default();
        let args = serde_json::json!({"command": "rm", "args": ["-rf", "/"]});
        assert!(matches!(
            policy.evaluate_tool("sandbox_exec", &args),
            PolicyDecision::Deny(_)
        ));
    }

    #[test]
    fn allows_echo() {
        let policy = ExecPolicy::default();
        let args = serde_json::json!({"command": "echo", "args": ["hello"]});
        assert_eq!(
            policy.evaluate_tool("sandbox_exec", &args),
            PolicyDecision::Allow
        );
    }

    #[test]
    fn exempt_tools_skip_policy() {
        let policy = ExecPolicy::default();
        assert_eq!(
            policy.evaluate_tool("list_dir", &serde_json::json!({"path": "."})),
            PolicyDecision::Allow
        );
    }
}
