# Architecture

## Design Philosophy

**Model + Harness = Agent**

The Harness layer sits between the application and the LLM model. It is responsible for:

1. **Agent Loop** — the perceive-reason-act-observe cycle
2. **Tool Use** — registering, selecting, and executing tools
3. **Context Engineering** — managing the context window efficiently
4. **Memory** — short-term (session) and long-term (episodic) memory
5. **Trace** — recording every step for debugging, evaluation, and model co-evolution

## Agent Loop (ReAct)

```
User Input
    │
    ▼
┌─→ LLM.complete(messages, tools)
│       │
│       ├── FinalAnswer → return
│       │
│       └── ToolCall → execute tool → append observation
│               │
└───────────────┘ (repeat until done or max_iterations)
```

## Module Dependencies

```
harness-core
    ├── harness-tools (ToolRegistry)
    ├── harness-mcp (MCP Client → Tool bridge)
    ├── harness-sandbox (ProcessSandbox)
    ├── harness-models (ModelProvider)
    └── harness-trace (Tracer)

harness-multi
    ├── harness-core
    └── harness-tools

coding-agent (example)
    ├── harness-core
    ├── harness-tools
    ├── harness-models
    └── harness-trace
```

## Context Engineering Strategy

| Layer | Content | Management |
|-------|---------|------------|
| System | Fixed instructions, tool schemas | Static |
| Working | Recent conversation turns | Sliding window |
| Tool Results | Observations from tool calls | Truncate large outputs |
| Episodic | Cross-session summaries | Vector retrieval (planned) |

## Evaluation Dimensions

- Task completion rate
- Tool call accuracy (correct tool + valid params)
- Multi-turn coherence
- Loop efficiency (iterations / tokens used)
