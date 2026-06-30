# MCP Server Configuration

Pick supports the Model Context Protocol (MCP) to connect external servers for extended tool capabilities.

## Transport types

| Transport | Protocol | Description |
|-----------|----------|-------------|
| `stdio` | stdin/stdout | Local subprocess communication |
| `SSE` | Server-Sent Events | HTTP streaming push |
| `HTTP` | Streamable HTTP | HTTP request/response |

## Configuration

MCP servers are configured in `settings.json`:

```json
{
  "mcp_servers": {
    "<server-name>": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "."],
      "env": { "NODE_ENV": "production" },
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
| `disabled` | No | Whether disabled (default false) |

### Examples

**Stdio servers:**

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
    "env": { "GITHUB_TOKEN": "ghp_your_token_here" },
    "tool_name_prefix": "gh_"
  }
}
```

**HTTP/SSE:**

```json
{
  "internal-api": {
    "url": "https://api.internal.company.com/mcp",
    "auth": { "type": "bearer", "token": "sk-your-token" },
    "tool_name_prefix": "api_"
  }
}
```

**Docker:**

```json
{
  "docker": {
    "command": "docker",
    "args": ["run", "-i", "--rm", "mcp/postgres-server"],
    "env": { "DATABASE_URL": "postgresql://..." }
  }
}
```

A complete example config is at `examples/settings-mcp-example.json`.

## Tool name prefix

Avoids naming conflicts between servers. A filesystem server's `read` tool becomes `fs_read` with prefix set.

## Security recommendations

1. **Least privilege** — Grant MCP servers minimal necessary scope
2. **Environment variables** — Store sensitive data (tokens, passwords) in the `env` field
3. **Container isolation** — Run untrusted MCP servers in Docker
4. **Network restrictions** — Only configure SSE/HTTP for necessary APIs
5. **Verify commands** — Ensure stdio commands come from trusted sources
