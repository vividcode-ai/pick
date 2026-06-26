# Tool System

Pick provides a set of built-in tools that can be extended via extensions and MCP servers.

## Built-in tools

| Tool | Description |
|------|-------------|
| `read` | Read file contents with line range and image preview support |
| `write` | Create or overwrite files |
| `edit` | Apply precise text replacements in files |
| `bash` | Execute shell commands (sandboxed) |
| `grep` | Search for patterns in file contents |
| `find` | Search for files by path patterns |
| `ls` | List directory contents |
| `webfetch` | Fetch web page content and convert to Markdown |
| `subagent` | Delegate tasks to specialized sub-agents |
| `question` | Ask the user a question |
| `todo_plan` | Track task progress |
| `get_goal` / `create_goal` / `update_goal` | Goal management (only available when `/goal` is active) |

## Tool allowlist

Restrict available tools with `-t` / `--tools`:

```bash
# Read-only mode
pick -P -t read,grep,find,ls "Analyze the architecture of this project"

# Disable bash and webfetch
pick -t read,write,edit,grep,find,ls
```

## Tool parameter schema

All tools use JSON Schema for parameter definitions:

```rust
AgentTool {
    name: "read",
    description: "Read file contents",
    parameters: JsonSchema {
        schema_type: "object",
        properties: Some({
            "file_path": { "type": "string" },
            "offset": { "type": "integer" },
            "limit": { "type": "integer" },
        }),
        required: Some(vec!["file_path"]),
    },
    execute: Arc::new(|tool_call_id, args, ctx| {
        Box::pin(async move {
            // Implementation
        })
    }),
}
```

## Execution modes

| Mode | Description |
|------|-------------|
| `Sequential` | Tools execute in order, one result at a time |
| `Concurrent` | Tools can execute in parallel |

## Extending tools

Tools can be extended through:

1. **[Extension system](extensions.md)** — Register custom tools via `register_tool()`
2. **[MCP servers](mcp.md)** — Register external tools via the Model Context Protocol
3. **Tool overrides** — Extensions can register tools with the same name to replace default implementations
