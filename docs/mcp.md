# MCP Server Configuration

Pick supports the Model Context Protocol (MCP), allowing connection to external MCP servers to extend tool capabilities.

## Transport types

| Transport | Protocol | Description |
|-----------|----------|-------------|
| `stdio` | stdin/stdout | Local subprocess communication |
| `SSE` | Server-Sent Events | HTTP streaming push |
| `HTTP` | Streamable HTTP | HTTP request/response |

## Configuration

MCP servers are configured in settings.json:

```json
{
  "mcp_servers": {
    "<server-name>": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "."],
      "env": {
        "NODE_ENV": "production"
      },
      "tool_name_prefix": "fs_",
      "disabled": false
    }
  }
}
```

### Configuration fields

| Field | Required | Description |
|-------|----------|-------------|
| `command` | ✓ (stdio) | Start command |
| `args` | No | Command arguments |
| `env` | No | Environment variable overrides |
| `tool_name_prefix` | No | Tool name prefix (e.g. `fs_read`) |
| `url` | ✓ (HTTP/SSE) | Server URL |
| `auth` | No | Authentication details |
| `disabled` | No | Whether server is disabled (default false) |

### Stdio example

```json
{
  "filesystem": {
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/workspace"],
    "tool_name_prefix": "fs_"
  },
  "github": {
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-github"],
    "env": {
      "GITHUB_TOKEN": "ghp_your_token_here"
    },
    "tool_name_prefix": "gh_"
  },
  "playwright": {
    "command": "npx",
    "args": ["-y", "@playwright/mcp"],
    "env": {
      "PLAYWRIGHT_BROWSER_PATH": "/usr/bin/chromium"
    }
  }
}
```

### HTTP/SSE example

```json
{
  "internal-api": {
    "url": "https://api.internal.company.com/mcp",
    "auth": {
      "type": "bearer",
      "token": "sk-your-token"
    },
    "tool_name_prefix": "api_"
  }
}
```

### Docker example

```json
{
  "docker": {
    "command": "docker",
    "args": ["run", "-i", "--rm", "mcp/postgres-server"],
    "env": {
      "DATABASE_URL": "postgresql://..."
    }
  }
}
```

## Example config

A complete example config file is available at `examples/settings-mcp-example.json`.

## Tool name prefix

`tool_name_prefix` avoids naming conflicts between tools. For example, a filesystem server's `read` tool becomes `fs_read` when a prefix is set.

## Security recommendations

1. **Least privilege** — Grant MCP servers only the necessary scope
2. **Environment variables** — Store sensitive information (tokens, passwords) in the `env` field
3. **Container isolation** — Consider running untrusted MCP servers in Docker
4. **Network restrictions** — Only configure SSE/HTTP transport for necessary APIs
5. **Verify commands** — Ensure stdio commands come from trusted sources
