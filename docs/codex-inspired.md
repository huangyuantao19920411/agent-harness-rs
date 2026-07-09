# Codex-Inspired Harness Features

This document describes features borrowed from [OpenAI Codex](https://github.com/openai/codex) and how they map to `agent-harness-rs`.

## Architecture Mapping

| Codex (codex-rs) | agent-harness-rs | Status |
|------------------|------------------|--------|
| Agent Loop (turn-based) | `harness-core::AgentLoop` | ✅ |
| ToolRouter (policy → approval → exec) | `harness-core::ToolOrchestrator` | ✅ |
| Exec policy (allow/deny patterns) | `harness-core::ExecPolicy` | ✅ |
| Context compaction | `harness-core::ContextManager` + `compaction` | ✅ LLM + heuristic fallback |
| App Server (JSON-RPC stdio) | `harness-app-server` | ✅ |
| MCP tool bridge | `harness-mcp` | ✅ |
| Sandbox (platform isolation) | `harness-sandbox` (Process/Wasm/K8s) | ✅ |
| Unified exec (long-lived workers) | `harness-sandbox::ExecProcessManager` | ✅ |
| Multi-agent threads | `harness-multi::MultiAgentRunner` | ✅ |
| Trace / replay | `harness-trace` JSONL | ✅ |
| Guardian approval (LLM reviewer) | `harness-core::GuardianReviewer` | ✅ |
| Memory pipeline (SQLite) | `harness-memory` | ✅ |
| Skills | `harness-tools::SkillRegistry` | ✅ |

## Tool Orchestrator Pipeline

Codex routes every tool call through:

```
ToolCall → ExecPolicy → ApprovalHandler → Sandbox → Execute
```

Our implementation:

```rust
let orchestrator = ToolOrchestrator::new(tools, ExecPolicy::default())
    .with_approval(Arc::new(AutoApprove));
```

Policy-denied commands return `[policy denied: ...]` as tool observation (agent can recover), matching Codex's "retry with feedback" pattern.

## Exec Policy

Default deny patterns (inspired by Codex execpolicy):

- `rm -rf`, `curl | sh`, fork bombs
- Allow: `echo`, `ls`, `cargo`, `git status/diff/log`

Configure via `HarnessConfig::exec_policy`.

## App Server Protocol

JSON-RPC 2.0 over stdio (like Codex App Server):

```bash
cargo run -p app-server
```

| Method | Description |
|--------|-------------|
| `initialize` | Handshake, capabilities |
| `thread/start` | Create agent thread |
| `turn/submit` | Run one agent turn |
| `thread/list` | List active threads |

Example:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"thread/start","params":{"trace_path":"/tmp/trace.jsonl"}}
{"jsonrpc":"2.0","id":3,"method":"turn/submit","params":{"thread_id":"<id>","input":"hello"}}
```

## Context Compaction (LLM-driven)

Inspired by Codex's compaction endpoint: when estimated tokens exceed `trigger_ratio × max_context_tokens`, the harness calls the model with a dedicated summarization prompt (no tools) before the next agent turn.

```
Agent turn N → [tokens > 75% budget?] → LLM compact → summary + recent tail → continue
```

Configure via `HarnessConfig::compaction`:

```rust
HarnessConfig {
    max_context_tokens: 8192,
    compaction: CompactionConfig {
        enabled: true,
        trigger_ratio: 0.75,       // compact at 75% budget
        keep_recent_messages: 6,     // preserve latest tool results
        fallback_heuristic: true,    // sliding window if LLM fails
    },
    ..Default::default()
}
```

Compaction events are recorded in trace as `ContextCompacted`.

## Guardian Approval (LLM reviewer)

Inspired by Codex Guardian: after rule-based `ExecPolicy`, unknown shell commands trigger a separate LLM review (no tools):

```
sandbox_exec → ExecPolicy → NeedsApproval → Guardian LLM → APPROVE/DENY → execute or reject
```

```rust
HarnessConfig {
    exec_policy: ExecPolicy {
        mode: ApprovalMode::Prompt,  // unknown shell → needs approval
        ..ExecPolicy::default()
    },
    guardian: GuardianConfig {
        enabled: true,
        review_allowlisted: false,  // strict: also review allowlisted commands
        review_unknown_tools: false,
    },
    ..Default::default()
}
```

Guardian responds with structured output:

```
DECISION: APPROVE
REASON: read-only ls command
```

Trace event: `ToolApprovalReview { approved, reviewer, reason }`.

Disable for CI/demo: `GUARDIAN_DISABLED=1 cargo run -p coding-agent -- "..."`

## Memory Pipeline (SQLite)

Codex-inspired two-phase episodic memory:

```
Session start → recall from SQLite → inject into system prompt
Session end   → LLM extract facts  → persist to SQLite
```

```bash
MEMORY_PATH=.agent/memory.db SESSION_ID=my-project cargo run -p coding-agent -- "continue yesterday's task"
```

Configure via `HarnessConfig::memory` or `MemoryConfig::from_env()`:

```rust
memory: MemoryConfig {
    enabled: true,
    db_path: ".agent/memory.db".into(),
    max_recall: 8,
    global_recall: true,       // cross-session recall
    extract_on_complete: true,
    max_extract: 5,
}
```

Memory kinds: `fact`, `preference`, `task`, `error`.

Trace events: `MemoryRecalled`, `MemoryPersisted`.

## K8s Sandbox (kube-rs)

In-cluster sandbox Jobs use the Kubernetes API directly via `kube` crate — no `kubectl` binary required:

```
untrusted task → SandboxScheduler → kube-rs create Job → wait → logs → delete
                                  ↘ kubectl fallback (local dev)
```

```bash
SANDBOX_K8S_BACKEND=kube SANDBOX_RUNTIME_CLASS=gvisor cargo run -p sandbox-demo
```

## Unified Exec Process Manager

Codex-style long-lived shell workers for trusted/code tasks — reuse one `sh` process across multiple commands:

```
trusted task → SandboxScheduler → ExecProcessManager → managed sh worker
                                              ↘ one-shot ProcessSandbox (default)
```

```bash
# Enable managed shell workers (reuse process_id across calls)
SANDBOX_EXEC_MODE=managed cargo run -p sandbox-demo

# Limit worker pool size (default 4)
SANDBOX_EXEC_MAX_PROCESSES=8 cargo run -p coding-agent -- "run tests twice"
```

Configure via environment:

| Variable | Values | Default |
|----------|--------|---------|
| `SANDBOX_EXEC_MODE` | `oneshot`, `managed`, `pty` | `oneshot` |
| `SANDBOX_EXEC_MAX_PROCESSES` | integer | `4` |

## Agent Skills (progressive disclosure)

Codex/Cursor-compatible `SKILL.md` discovery with on-demand loading:

```
Startup → scan .agents/skills, .cursor/skills → inject catalog (name + description)
Agent task matches skill → load_skill tool → full instructions loaded
```

```bash
SKILLS_ENABLED=1 cargo run -p coding-agent -- "Review this Rust PR"
```

Skills layout: `.agents/skills/rust-review/SKILL.md`

Trace event: `SkillLoaded { name, path }`.

## PTY Sandbox (pseudo-terminal)

TTY-aware execution for colors, progress bars, and interactive programs:

```bash
SANDBOX_EXEC_MODE=pty cargo run -p sandbox-demo
```

PTY workers run each command in a fresh pseudo-terminal (TTY-aware output).

| Mode | Behavior |
|------|----------|
| `oneshot` | Fresh subprocess per command (default) |
| `managed` | Long-lived shell via pipes (reuse `process_id`) |
| `pty` | Fresh PTY per command (colors, progress bars) |

## Next Steps (Codex parity)

1. ~~LLM-based context compaction~~ ✅
2. ~~Guardian-style LLM approval reviewer~~ ✅
3. ~~Persistent memory (SQLite episodic store)~~ ✅
4. ~~`kube` crate instead of `kubectl` CLI for in-cluster sandbox~~ ✅
5. ~~Unified exec process manager (long-lived sandbox workers)~~ ✅
