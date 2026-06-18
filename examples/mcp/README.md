# MCP Server Configuration Examples

This directory contains example configurations for running MCP (Model Context Protocol) servers with Pick.

## Quick Start

Copy `settings-mcp-example.json` into your Pick settings:

**Global settings** (applies to all projects):
```
~/.pick/agent/settings.json
```

**Project settings** (overrides global, per-project):
```
your-project/.pick/settings.json
```

Or place `.pick/settings.json` from this directory into your project root.

## Configuration Reference

```jsonc
{
  "mcp_servers": {
    "<server-name>": {
      // --- Stdio transport (run process + talk via stdin/stdout) ---
      "command": "npx",           // executable to run
      "args": [
        "-y",
        "@modelcontextprotocol/server-filesystem",
        "/allowed/path"
      ],
      "env": {                    // optional environment variables
        "KEY": "value"
      },

      // --- HTTP transport (connect to remote server) ---
      "url": "https://example.com/mcp",
      "auth": {
        "type": "bearer",         // "bearer" | "oauth2"
        "token": "sk-xxx"
      },

      // --- Common options ---
      "tool_name_prefix": "fs_",  // prefix all tool names from this server
    }
  }
}
```

## Transport Types

| Transport | Config | Use Case |
|-----------|--------|----------|
| **Stdio** | `command` + `args` | Local servers (filesystem, git, DB, etc.) |
| **HTTP** | `url` + optional `auth` | Remote/cloud-hosted servers |

## Auth Types (HTTP only)

| Auth Type | Config | Description |
|-----------|--------|-------------|
| `bearer` | `{ "type": "bearer", "token": "..." }` | Static Bearer token |
| `oauth2` | `{ "type": "oauth2", "client_id": "...", "scopes": [...] }` | OAuth 2.1 (PKCE flow, not yet implemented) |

## Tool Name Prefix

When connecting multiple servers, tool names may collide. Use `tool_name_prefix`:

```json
{
  "mcp_servers": {
    "fs": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "."],
      "tool_name_prefix": "fs_"
    },
    "db": {
      "command": "npx",
      "args": ["-y", "mcp-server-sqlite", "--db-path", "app.db"],
      "tool_name_prefix": "db_"
    }
  }
}
```

With prefixes, the LLM sees tools named `fs_read`, `fs_write`, `db_query`, etc.

## Runtime Management

Once connected, use slash commands to manage servers at runtime:

| Command | Description |
|---------|-------------|
| `/mcp` | Show help |
| `/mcp list` | List connected servers and their tools |
| `/mcp connect <name> --command <cmd> [--args ...]` | Connect a new server |
| `/mcp disconnect <name>` | Disconnect a server |

## Popular MCP Servers

- **Filesystem**: `npx -y @modelcontextprotocol/server-filesystem <dirs...>`
- **GitHub**: `npx -y @modelcontextprotocol/server-github` (env: `GITHUB_TOKEN`)
- **Playwright**: `npx -y @playwright/mcp`
- **SQLite**: `uvx mcp-server-sqlite --db-path <path>`
- **Docker**: `docker run -i --rm mcp/docker-server`
- **Fetch**: `npx -y @modelcontextprotocol/server-fetch`
- **Sequential Thinking**: `npx -y @modelcontextprotocol/server-sequential-thinking`
