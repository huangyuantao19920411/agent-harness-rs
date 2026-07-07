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

## Features (Roadmap)

- [x] Agent Loop (ReAct)
- [x] Tool Registry + Function Calling
- [x] Execution Trace
- [x] Mock LLM provider
- [x] DeepSeek API adapter
- [x] MCP Client (stdio transport + tool bridge)
- [x] Process Sandbox (timeout, cwd jail, env filter)
- [ ] Wasm Sandbox (wasmtime)
- [ ] Context compression
- [ ] Multi-Agent orchestration
- [ ] K8s RuntimeClass integration (gVisor / Firecracker)
- [ ] K8s Helm chart

## Sandbox Strategy

See [docs/sandbox.md](docs/sandbox.md) for the full analysis. Summary:

| Phase | Backend | Use Case |
|-------|---------|----------|
| 1 (done) | Process | Demo, trusted tools |
| 2 | Wasm (wasmtime) | AI-generated code |
| 3 | gVisor / Firecracker via K8s | Production untrusted code |

We integrate existing isolation technologies rather than building a Firecracker clone.

## License

MIT
