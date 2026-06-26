# Settings

Pick uses a two-tier configuration merge: global settings provide defaults, project settings override for specific projects.

## Config locations

| Tier | Path | Description |
|------|------|-------------|
| Global | `~/.pick/settings.json` | Default config for all projects |
| Project | `.pick/settings.json` | Overrides global settings |

## Configuration reference

```json
{
  "default_provider": "anthropic",
  "default_model": "claude-sonnet-4-20250514",
  "thinking_level": "off",
  "permission": {
    "approval_policy": "on_request",
    "permission_profile": ":workspace"
  },
  "compaction": {
    "max_messages": 150,
    "max_tokens": 32000,
    "enabled": true
  },
  "session": {
    "dir": "~/.pick/agent/sessions",
    "max_history": 50
  },
  "theme": "catppuccin-mocha",
  "tools": {
    "allowlist": ["read", "bash", "edit", "write", "grep", "find", "ls"]
  },
  "extensions": {
    "dirs": ["~/.pick/extensions"]
  }
}
```

### General

| Field | Description |
|-------|-------------|
| `default_provider` | Default LLM provider |
| `default_model` | Default model ID |
| `thinking_level` | Thinking level: `off`, `minimal`, `low`, `medium`, `high`, `xhigh` |

### Permission

| Field | Type | Description |
|-------|------|-------------|
| `approval_policy` | string | `on_request` (ask on request) or `auto` (auto-approve safe operations) |
| `permission_profile` | string | Permission profile path or `:workspace` (restrict to workspace directory) |

### Compaction

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_messages` | int | 150 | Max messages before compaction triggers |
| `max_tokens` | int | 32000 | Max tokens before compaction triggers |
| `enabled` | bool | true | Enable automatic compaction |

### Tools

```json
{
  "tools": {
    "allowlist": ["read", "bash", "edit", "write"],
    "bash_exec_policy": {
      "known_safe": ["ls", "cat", "head", "tail", "pwd", "echo", "which"],
      "known_dangerous": ["rm", "sudo", "kill", "dd", "mkfs"],
      "rules_file": ".pick/rules/default.rules"
    }
  }
}
```

### MCP servers

MCP servers can be configured inline in settings or in a separate file. See [MCP docs](mcp.md) for details.

```json
{
  "mcp_servers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem"]
    }
  }
}
```

## Merge rules

1. Global config is loaded first
2. If project `.pick/settings.json` exists, it is merged on top of global config
3. For the same field, **project config overrides global config**
4. For array fields, the merge order is **global first, project second**
5. For object fields, deep merge is used (full object is not replaced)
