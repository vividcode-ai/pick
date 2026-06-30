# Architecture

Pick is a Rust workspace with 6 crates arranged in a dependency hierarchy.

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

```
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
Terminal UI built on crossterm + ratatui. Features custom differential rendering, Markdown rendering, syntax highlighting, image display, undo/redo, key bindings, and theme system.

### pick-ai
Unified multi-provider LLM abstraction with provider registry (Anthropic, OpenAI, Google, Mistral, Bedrock, etc.), streaming, thinking/reasoning, auto-retry, token counting, and context management.

### pick-agent
Core agent: input → LLM call → tool execution → output. Includes tool system (read, write, edit, bash, grep, find, ls, webfetch), session management (JSONL persistence, fork/resume, compaction, branch summarization), extension system (dynamic library loading), permission system (allow/deny/ask rules, audit), skill system (Markdown instructions), and sub-agents.

### pick-cli
Binary entrypoint with CLI argument parsing, run mode dispatch (TUI, interactive, print, json, rpc), settings loading (global + project merge), and auth management.

### pick-mcp
Model Context Protocol client supporting stdio, SSE, and streamable HTTP transports, with tool registration and auto-discovery.

### pick-sandbox
Platform-specific process isolation: Windows (restricted tokens + Job Objects), Linux (bubblewrap), macOS (Seatbelt). Falls back to null sandbox on failure.

## Data flow

```text
User Input → CLI Arg Parsing → AgentSession
    → AgentLoop → LLM provider → Tool Execution
    → Session Storage (JSONL) → UI Update
```

- [Tools](tools.md) — Built-in and extension tools
- [Extensions](extensions.md) — Lifecycle events and custom tools
- [Sessions](sessions.md) — Persistence and restore
