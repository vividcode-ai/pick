# Tool System

Built-in tools can be extended via extensions and MCP servers.

## Built-in tools

| Tool | Description |
|------|-------------|
| `read` | Read file contents with line range and image preview |
| `write` | Create or overwrite files |
| `edit` | Apply precise text replacements |
| `bash` | Execute shell commands (sandboxed) |
| `grep` | Search for patterns in file contents |
| `find` | Search for files by path patterns |
| `ls` | List directory contents |
| `webfetch` | Fetch web page content to Markdown |
| `subagent` | Delegate tasks to specialized sub-agents |
| `question` | Ask the user a question |
| `todo_plan` | Track task progress |
| `get_goal` / `create_goal` / `update_goal` | Goal management (when `/goal` active) |

## Tool allowlist

Restrict tools with `-t` / `--tools`:

```bash
# Read-only mode
pick -P -t read,grep,find,ls "Analyze this project"

# Disable bash and webfetch
pick -t read,write,edit,grep,find,ls
```

## Execution modes

| Mode | Description |
|------|-------------|
| `Sequential` | One tool at a time, in order |
| `Concurrent` | Tools execute in parallel |

## Extending tools

1. **[Extension system](extensions.md)** — Register custom tools via `register_tool()`
2. **[MCP servers](mcp.md)** — External tools via Model Context Protocol
3. **Tool overrides** — Extensions can replace default implementations by registering same-named tools
