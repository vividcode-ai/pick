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

## TUI Settings Menu (`/settings`)

Enter `/settings` in TUI mode to open the interactive settings menu. Changes are saved to the global config (`~/.pick/settings.json`) and take effect immediately (or on next agent run where noted).

### Toggle settings

| Menu item | Field | Description | Default |
|-----------|-------|-------------|---------|
| Auto-compact | `compaction.enabled` | Automatically compact conversation context when it exceeds the token limit | `true` |
| Sandbox | `permission.sandbox_enabled` | Run commands in a sandboxed/restricted environment | `false` |
| MCP tools | `enable_mcp_tools` | Enable MCP server tools for the agent to use (takes effect on next agent run) | `true` |
| System notifications | `enable_system_notifications` | Show OS-level system notifications for agent events (takes effect on next agent run) | `true` |
| Show images | `terminal.show_images` | Render images inline in terminal output | `true` |
| Auto-resize images | `images.auto_resize` | Automatically resize large images to 2000x2000px max for better model compatibility | `true` |
| Block images | `images.block_images` | Prevent images from being sent to LLM providers | `false` |
| Skill commands | `enable_skill_commands` | Register loaded skills as `/skill:name` slash commands and rebuild autocomplete | `false` |
| Show hardware cursor | `show_hardware_cursor` | Show the terminal cursor while positioning it for IME support | `false` |
| Clear on shrink | `terminal.clear_on_shrink` | Clear empty rows when terminal content shrinks (may cause flicker on some terminals) | `false` |
| Terminal progress | `terminal.show_terminal_progress` | Show OSC 9;4 progress indicators in the terminal tab/title bar | `false` |
| Show thinking | `hide_thinking_block` | Show or hide the model's thinking/reasoning blocks in responses | enabled |
| Collapse changelog | `collapse_changelog` | Show a condensed version of the changelog after updates | `false` |
| Quiet startup | `quiet_startup` | Suppress verbose startup messages | `false` |
| Install telemetry | `enable_install_telemetry` | Send an anonymous version/update ping after changelog-detected updates | `false` |

### Sub-selector settings

These settings open a sub-menu to pick from predefined values.

| Menu item | Field | Options | Description |
|-----------|-------|---------|-------------|
| Image width | `terminal.image_width_cells` | `60`, `80`, `120` | Preferred inline image width in terminal cells |
| Editor padding | `editor_padding_x` | `0`, `1`, `2`, `3` | Horizontal padding (in cells) around the input editor |
| Autocomplete max items | `autocomplete_max_visible` | `3`, `5`, `7`, `10`, `15`, `20` | Maximum number of visible items in the autocomplete dropdown |
| Steering mode | `steering_mode` | `one-at-a-time`, `all` | How steering messages are queued while the agent is streaming |
| Follow-up mode | `follow_up_mode` | `one-at-a-time`, `all` | How follow-up messages are queued until the agent stops |
| Transport | `transport` | `sse`, `websocket`, `websocket-cached`, `auto` | Preferred transport for providers that support multiple transports |
| HTTP idle timeout | `http_idle_timeout_ms` | `30s`, `1min`, `5min`, `10min`, `30min`, `Disabled` | Maximum idle time while waiting for HTTP headers or body chunks |
| Double-escape action | `double_escape_action` | `tree`, `fork`, `none` | Action when pressing Escape twice with an empty editor |
| Tree filter mode | `tree_filter_mode` | `default`, `no-tools`, `user-only`, `labeled-only`, `all` | Default filter when opening the `/tree` command |
| Warnings | `warnings` | (submenu) | Toggle individual warnings (e.g., Anthropic extra usage) |
| Thinking level | `default_thinking_level` | `off`, `minimal`, `low`, `medium`, `high`, `xhigh` | Reasoning depth for thinking-capable models. Available levels depend on the selected model |
| Theme | `theme` | `dark`, `light`, `solarized-dark`, `solarized-light` | Color theme for the TUI interface (requires restart) |

#### Thinking level details

| Level | Token budget | Use case |
|-------|-------------|----------|
| `off` | 0 | Fastest, cheapest responses; no reasoning |
| `minimal` | ~1k tokens | Very brief reasoning for simple tasks |
| `low` | ~2k tokens | Light reasoning for straightforward questions |
| `medium` | ~8k tokens | Moderate reasoning for most development tasks |
| `high` | ~16k tokens | Deep reasoning for complex bugs or design decisions |
| `xhigh` | ~32k tokens | Maximum reasoning for the hardest problems |

### Model selector

The **Models** option opens a selector to change the default model for the current provider. This updates both `default_provider` and `default_model` in global settings.

### Settings exposed via CLI flags

Some settings can be overridden at startup via CLI flags instead of the settings menu:

| Flag | Overrides |
|------|-----------|
| `--model` / `-m` | `default_model` |
| `--provider` / `-p` | `default_provider` |
| `--thinking <LEVEL>` | `default_thinking_level` |
| `--theme <THEME>` | `theme` |
| `--session-dir` | `session_dir` |
| `--extension` / `-e` | `extensions` (appends to list)
