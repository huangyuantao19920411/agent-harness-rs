# agent-harness-rs

Open-source **Agent Harness** framework in Rust.

> **Model + Harness = Agent**

A reference implementation of the Agent Harness layer: everything beyond the LLM that makes an Agent work in production — Agent Loop, Tool Use, MCP, Context Engineering, Multi-Agent orchestration, and execution tracing.

## Architecture

```
┌─────────────────────────────────────────────┐
│              Application Layer               │
│         (coding-agent, research-agent)       │
├─────────────────────────────────────────────┤
│              Harness Layer                   │
│  ┌──────────┐ ┌─────────┐ ┌─────────────┐  │
│  │ harness- │ │ harness-│ │ harness-    │  │
│  │ core     │ │ tools   │ │ multi       │  │
│  │ (Loop)   │ │ (MCP)   │ │ (Subagent)  │  │
│  └────┬─────┘ └────┬────┘ └──────┬──────┘  │
│       └────────────┼─────────────┘          │
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
```

When `DEEPSEEK_API_KEY` is set, `coding-agent` automatically uses the DeepSeek adapter (`deepseek-chat` by default).

Optional environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `DEEPSEEK_API_KEY` | - | DeepSeek API key |
| `DEEPSEEK_MODEL` | `deepseek-chat` | Model name (future) |
| `DEEPSEEK_BASE_URL` | `https://api.deepseek.com` | API base URL (future) |

## Features (Roadmap)

- [x] Agent Loop (ReAct)
- [x] Tool Registry + Function Calling
- [x] Execution Trace
- [x] Mock LLM provider
- [x] DeepSeek API adapter
- [ ] MCP Server/Client
- [ ] Context compression
- [ ] Multi-Agent orchestration
- [ ] K8s Helm chart

## License

MIT
