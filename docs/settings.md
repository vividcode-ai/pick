# Settings

Two-tier configuration: global defaults, project overrides.

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
| `thinking_level` | `off`, `minimal`, `low`, `medium`, `high`, `xhigh` |

### Permission

| Field | Type | Description |
|-------|------|-------------|
| `approval_policy` | string | `on_request` (ask) or `auto` (auto-approve safe ops) |
| `permission_profile` | string | Profile path or `:workspace` (restrict to workspace) |

### Compaction

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_messages` | int | 150 | Max messages before compaction |
| `max_tokens` | int | 32000 | Max tokens before compaction |
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

See [MCP docs](mcp.md) for details.

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

1. Global config loaded first
2. Project `.pick/settings.json` merged on top
3. **Project config overrides global** for same field
4. Array fields: global first, project second
5. Object fields: deep merge (full object not replaced)

## TUI Settings Menu (`/settings`)

Enter `/settings` in TUI mode to open the interactive menu. Changes save to `~/.pick/settings.json` and take effect immediately (or on next run where noted).

### Toggle settings

| Menu item | Field | Description | Default |
|-----------|-------|-------------|---------|
| Auto-compact | `compaction.enabled` | Auto-compact conversation context | `true` |
| Sandbox | `permission.sandbox_enabled` | Run commands in sandboxed environment | `false` |
| MCP tools | `enable_mcp_tools` | Enable MCP server tools (next run) | `true` |
| System notifications | `enable_system_notifications` | Show OS notifications for agent events (next run) | `true` |
| Show images | `terminal.show_images` | Render images inline | `true` |
| Auto-resize images | `images.auto_resize` | Resize large images to 2000x2000px max | `true` |
| Block images | `images.block_images` | Prevent images sent to LLM | `false` |
| Skill commands | `enable_skill_commands` | Register skills as `/skill:name` commands | `false` |
| Show hardware cursor | `show_hardware_cursor` | Show terminal cursor for IME support | `false` |
| Clear on shrink | `terminal.clear_on_shrink` | Clear empty rows on shrink (may flicker) | `false` |
| Terminal progress | `terminal.show_terminal_progress` | Show OSC 9;4 progress indicators | `false` |
| Show thinking | `hide_thinking_block` | Show/hide thinking/reasoning blocks | enabled |
| Collapse changelog | `collapse_changelog` | Condensed changelog after updates | `false` |
| Quiet startup | `quiet_startup` | Suppress verbose startup messages | `false` |
| Install telemetry | `enable_install_telemetry` | Anonymous version ping after updates | `false` |

### Sub-selector settings

| Menu item | Field | Options | Description |
|-----------|-------|---------|-------------|
| Image width | `terminal.image_width_cells` | `60`, `80`, `120` | Inline image width in cells |
| Editor padding | `editor_padding_x` | `0`, `1`, `2`, `3` | Horizontal padding around input editor |
| Autocomplete max | `autocomplete_max_visible` | `3`, `5`, `7`, `10`, `15`, `20` | Max visible autocomplete items |
| Steering mode | `steering_mode` | `one-at-a-time`, `all` | How steering messages queue while streaming |
| Follow-up mode | `follow_up_mode` | `one-at-a-time`, `all` | How follow-ups queue until agent stops |
| Transport | `transport` | `sse`, `websocket`, `websocket-cached`, `auto` | Provider transport preference |
| HTTP idle timeout | `http_idle_timeout_ms` | `30s`, `1min`, `5min`, `10min`, `30min`, `Disabled` | Max idle time for HTTP |
| Double-escape action | `double_escape_action` | `tree`, `fork`, `none` | Action on Escape x2 with empty editor |
| Tree filter mode | `tree_filter_mode` | `default`, `no-tools`, `user-only`, `labeled-only`, `all` | Default `/tree` filter |
| Warnings | `warnings` | (submenu) | Toggle individual warnings |
| Thinking level | `default_thinking_level` | `off`, `minimal`, `low`, `medium`, `high`, `xhigh` | Reasoning depth (depends on model) |
| Theme | `theme` | `dark`, `light`, `solarized-dark`, `solarized-light` | TUI color theme (requires restart) |

#### Thinking level details

| Level | Token budget | Use case |
|-------|-------------|----------|
| `off` | 0 | Fastest, cheapest |
| `minimal` | ~1k | Brief reasoning |
| `low` | ~2k | Light reasoning |
| `medium` | ~8k | Most dev tasks |
| `high` | ~16k | Complex bugs/design |
| `xhigh` | ~32k | Hardest problems |

### Model selector

Opens a selector to change the default model for the current provider. Updates both `default_provider` and `default_model`.

### CLI flag overrides

| Flag | Overrides |
|------|-----------|
| `--model` / `-m` | `default_model` |
| `--provider` / `-p` | `default_provider` |
| `--thinking <LEVEL>` | `default_thinking_level` |
| `--theme <THEME>` | `theme` |
| `--session-dir` | `session_dir` |
| `--extension` / `-e` | `extensions` (appends) |
