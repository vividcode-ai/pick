# CLI Reference

## Usage

```
pick [OPTIONS] [@files...] [MESSAGE...]
```

## Run modes

| Mode | Flag | Description |
|------|------|-------------|
| **TUI** (default) | `--mode tui` | Full terminal UI with diff rendering, syntax highlighting, images |
| **Interactive** | `--mode interactive` | REPL-style interaction |
| **Print** | `--mode print` / `-P` | Batch non-interactive, output to stdout |
| **JSON** | `--mode json` | JSON-formatted output |

## Options

### Model & provider

| Flag | Description |
|------|-------------|
| `-m, --model <MODEL>` | Model ID |
| `-p, --provider <PROV>` | Provider (anthropic, openai, google, etc.) |
| `--thinking <LEVEL>` | Reasoning level: off / minimal / low / medium / high / xhigh |
| `--list-models [FILTER]` | List available models |
| `--api-key <KEY>` | Set API key |

### Session

| Flag | Description |
|------|-------------|
| `-s, --session <ID>` | Resume session by ID |
| `-r, --resume` | Interactive session selector |
| `--fork <ID>` | Fork a session (snapshot then continue) |
| `-c, --continue [ID]` | Continue most recent or specified session |
| `--no-session` | Run without persistence |
| `--export <FILE>` | Export session to HTML |

### Agent behavior

| Flag | Description |
|------|-------------|
| `--agent-mode <MODE>` | `build` (default, can modify files) or `plan` (read-only research) |
| `--system-prompt <TEXT>` | Custom system prompt |
| `-t, --tools <TOOLS>` | Tool allowlist (comma-separated) |
| `-nt, --no-tools` | Disable all tools |
| `-nbt, --no-builtin-tools` | Disable built-in tools only |

### Extensions & customization

| Flag | Description |
|------|-------------|
| `-e, --extension <PATH>` | Load an extension |
| `--skill <PATH>` | Load a skill file |
| `-ne, --no-extensions` | Disable extension loading |
| `-ns, --no-skills` | Disable skills |
| `--no-themes` | Disable themes |
| `-nc, --no-context-files` | Disable context files |

### Audit

| Flag | Description |
|------|-------------|
| `--audit` | View permission audit trail |
| `--json` | JSON-format audit output |
| `--recent <N>` | N most recent audit entries |
| `--tool <NAME>` | Filter by tool name |
| `--decision <TYPE>` | Filter by decision (allow / deny / ask) |
| `--layer <LAYER>` | Filter by audit layer |

### Other

| Flag | Description |
|------|-------------|
| `-v, --verbose` | Verbose output |
| `--offline` | Offline mode |
| `--update` | Self-update binary |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

## Positional arguments

| Argument | Description |
|----------|-------------|
| `@files...` | Context files (e.g. `@src/main.rs`) |
| `MESSAGE...` | Input prompt (use with `-P` in print mode) |

## Exit codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | General error |
| 2 | Argument parsing error |
| 3 | Auth/configuration error |

## Environment variables

| Variable | Description |
|----------|-------------|
| `PICK_API_KEY` | Default API key |
| `PICK_PROVIDER` | Default provider (overrides settings.json) |
| `PICK_MODEL` | Default model (overrides settings.json) |
| `PICK_HOME` | Alternate config directory for `~/.pick` |
| `PICK_PACKAGE_DIR` | Alternate package resource directory |
