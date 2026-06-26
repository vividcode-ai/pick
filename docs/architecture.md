# Architecture

Pick is a Rust workspace project composed of 6 independent crates arranged in a dependency hierarchy.

## Workspace structure

```
pick/
├── crates/
│   ├── tui/          # Terminal UI (no deps on other pick crates)
│   ├── ai/           # LLM abstraction layer (no deps on other pick crates)
│   ├── agent/        # Agent loop, tools, sessions, extensions (depends on ai, tui)
│   ├── cli/          # Binary entrypoint (depends on all crates)
│   ├── mcp/          # MCP protocol client (depends on agent, ai)
│   └── sandbox/      # Process isolation (depends on agent)
└── docs/             # Documentation
```

## Dependency hierarchy

```text
pick-tui (crossterm/ratatui, pure UI)
    ↑
pick-ai (multi-provider LLM abstraction)
    ↑
pick-agent (agent core: loop, tools, sessions, extensions, permissions)
    ↑
pick-cli (binary entrypoint, arg parsing, mode dispatch)
```

```
pick-mcp (MCP client: stdio, SSE, HTTP)
pick-sandbox (process sandbox: Windows Job Objects, Linux bwrap, macOS Seatbelt)
    ↑
Both depend on pick-agent interfaces
```

## Crate overview

### pick-tui
- Terminal UI built on crossterm + ratatui
- Custom differential rendering engine
- Markdown rendering, syntax highlighting, image display
- Undo/redo, key bindings, theme system

### pick-ai
- Unified multi-provider LLM abstraction layer
- Provider registry pattern: Anthropic, OpenAI, Google, Mistral, Bedrock, and more
- Streaming responses, thinking/reasoning, auto-retry
- Token counting and context management

### pick-agent
- Agent main loop: user input → LLM call → tool execution → result output
- Tool system: read, write, edit, bash, grep, find, ls, webfetch
- Session management: JSONL persistence, fork/resume, compaction, branch summarization
- Extension system: dynamic library loading (libloading), lifecycle events
- Permission system: allow/deny/ask rules, audit log, execution policy
- Skill system: reusable Markdown instructions
- Sub-agents: task delegation to specialized agent processes

### pick-cli
- Binary entrypoint
- CLI argument parsing (clap)
- Run mode dispatch (TUI, interactive, print, json, rpc)
- Settings loading (global + project two-tier merge), auth management

### pick-mcp
- Model Context Protocol client implementation
- Supports stdio, SSE, streamable HTTP transports
- Tool registration and auto-discovery

### pick-sandbox
- Platform-specific process isolation
- Windows: restricted tokens + Job Objects
- Linux: bubblewrap
- macOS: Seatbelt
- Falls back to null (no sandbox) on failure

## Data flow

```text
User Input → CLI Arg Parsing → AgentSession
    → AgentLoop → LLM provider → Tool Execution
    → Session Storage (JSONL) → UI Update
```

- [Tools](tools.md) — Built-in and extension tools
- [Extensions](extensions.md) — Lifecycle events and custom tools
- [Sessions](sessions.md) — Persistence and restore
