# agent-harness-rs

Open-source **Agent Harness** framework in Rust.

> **Model + Harness = Agent**

A reference implementation of the Agent Harness layer: everything beyond the LLM that makes an Agent work in production — Agent Loop, Tool Use, MCP, Context Engineering, Multi-Agent orchestration, sandboxed execution, and execution tracing.

## Architecture

```
┌─────────────────────────────────────────────┐
│              Application Layer               │
│         (coding-agent, research-agent)       │
├─────────────────────────────────────────────┤
│              Harness Layer                   │
│  ┌──────────┐ ┌─────────┐ ┌─────────────┐  │
│  │ harness- │ │ harness-│ │ harness-    │  │
│  │ core     │ │ mcp     │ │ multi       │  │
│  │ (Loop)   │ │ (MCP)   │ │ (Subagent)  │  │
│  └────┬─────┘ └────┬────┘ └──────┬──────┘  │
│  ┌────┴─────┐ ┌────┴────┐               │
│  │ harness- │ │ harness-│               │
│  │ tools    │ │ sandbox │               │
│  └──────────┘ └─────────┘               │
│              harness-trace                   │
├────────────────────┼────────────────────────┤
│              harness-models                  │
│         (DeepSeek / OpenAI adapters)         │
└────────────────────┼────────────────────────┘
                     │ LLM API
              ┌──────▼──────┐
              │    Model     │
              └─────────────┘
```

## Crates

| Crate | Description |
|-------|-------------|
| `harness-core` | Agent Loop (ReAct), Context Engineering |
| `harness-tools` | Tool Registry, Function Calling |
| `harness-mcp` | MCP protocol client, remote tool bridge |
| `harness-sandbox` | Sandboxed execution (Process / Wasm / MicroVM roadmap) |
| `harness-trace` | Execution trace recording and replay |
| `harness-models` | LLM provider adapters (DeepSeek, mock) |
| `harness-multi` | Subagent delegation, Multi-Agent orchestration |
| `harness-app-server` | JSON-RPC App Server (Codex-inspired stdio protocol) |
| `harness-memory` | Episodic memory (SQLite extract + recall) |

## Quick Start

```bash
cargo build

# Mock model (no API key required)
cargo run -p coding-agent -- "List files in current directory"

# DeepSeek API (OpenAI-compatible)
export DEEPSEEK_API_KEY=your_api_key
cargo run -p coding-agent -- "List files in current directory"

# With MCP server tools (stdio transport)
export MCP_SERVER_COMMAND="npx"
export MCP_SERVER_ARGS="-y @modelcontextprotocol/server-filesystem /tmp"
cargo run -p coding-agent -- "What files are in /tmp?"

# Sandbox scheduler demo (Process / Wasm / K8s)
cargo run -p sandbox-demo
SKIP_K8S_SANDBOX=1 cargo run -p sandbox-demo   # skip K8s if no cluster

# App Server (JSON-RPC over stdio, Codex-inspired)
cargo run -p app-server

# Persist trace to JSONL
TRACE_PATH=/tmp/agent-trace.jsonl cargo run -p coding-agent -- "List files"
```

When `DEEPSEEK_API_KEY` is set, `coding-agent` automatically uses the DeepSeek adapter (`deepseek-chat` by default).

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DEEPSEEK_API_KEY` | - | DeepSeek API key |
| `DEEPSEEK_MODEL` | `deepseek-chat` | Model name |
| `DEEPSEEK_BASE_URL` | `https://api.deepseek.com` | API base URL |
| `MCP_SERVER_COMMAND` | - | MCP server executable |
| `MCP_SERVER_ARGS` | - | MCP server arguments (space-separated) |
| `SANDBOX_RUNTIME_CLASS` | `gvisor` | K8s RuntimeClass for MicroVM sandbox |
| `SANDBOX_NAMESPACE` | `agent-sandbox` | K8s namespace for sandbox Jobs |
| `SANDBOX_K8S_BACKEND` | `kube` | K8s backend: `kube` (in-cluster API) or `kubectl` |
| `SANDBOX_EXEC_MODE` | `oneshot` | Exec backend: `oneshot`, `managed`, or `pty` |
| `SANDBOX_EXEC_MAX_PROCESSES` | `4` | Max concurrent managed shell workers |
| `SKILLS_ENABLED` | `false` | Enable agent skills discovery |
| `SKILLS_PATH` | — | Extra skill search paths (`:` separated); also enables skills |
| `TRACE_PATH` | - | JSONL trace output path |
| `GUARDIAN_DISABLED` | - | Disable Guardian LLM review (permissive mode) |
| `MEMORY_PATH` | - | SQLite path for episodic memory (enables memory pipeline) |
| `SESSION_ID` | auto UUID | Session id for memory recall / persistence |

See [docs/codex-inspired.md](docs/codex-inspired.md) for Codex architecture mapping.

## Features (Roadmap)

- [x] Agent Loop (ReAct)
- [x] Tool Registry + Function Calling
- [x] Execution Trace
- [x] Mock LLM provider
- [x] DeepSeek API adapter
- [x] MCP Client (stdio transport + tool bridge)
- [x] Process Sandbox (timeout, cwd jail, env filter)
- [x] Wasm Sandbox (wasmtime, fuel-limited)
- [x] K8s RuntimeClass integration (gVisor / Kata + SandboxScheduler)
- [x] Exec policy + ToolOrchestrator + Guardian LLM review (Codex-inspired)
- [x] Context compression (LLM compaction + heuristic fallback)
- [x] Trace JSONL persistence + replay
- [x] App Server (JSON-RPC stdio)
- [ ] Multi-Agent orchestration (basic runner done, LLM planner pending)
- [x] K8s Helm chart + RBAC

## Sandbox Strategy

See [docs/sandbox.md](docs/sandbox.md) for the full analysis. Summary:

| Phase | Backend | Use Case |
|-------|---------|----------|
| 1 (done) | Process | Demo, trusted tools |
| 2 (done) | Wasm (wasmtime) | AI-generated code |
| 3 (done) | gVisor / Firecracker via K8s | Production untrusted code |

We integrate existing isolation technologies rather than building a Firecracker clone.

## 关联项目

- [agent-handbook](https://github.com/huangyuantao19920411/agent-handbook) — Agent 生态概念图解学习手册

## License

MIT
