# Quickstart

## Installation

### npm (recommended)

```bash
npm install -g @vividcodeai/pick
```

### Linux / macOS

```bash
curl -fsSL https://github.com/vividcode-ai/pick/releases/latest/download/install.sh | sh
```

### Windows (PowerShell)

```powershell
irm https://github.com/vividcode-ai/pick/releases/latest/download/install.ps1 | iex
```

### Build from source

```bash
git clone https://github.com/vividcode-ai/pick.git
cd pick
cargo build --release
./target/release/pick --help
```

## Quick start

```bash
# Start TUI (default mode)
pick

# Specify model and provider
pick -m claude-sonnet-4-20250514 -p anthropic

# One-shot question (print mode)
pick -P "What is the directory structure of this project?"

# Interactive REPL mode
pick --mode interactive

# List available models
pick --list-models

# Plan mode (read-only research)
pick --agent-mode plan -P "How should I refactor this module?"
```

## Initial setup

### 1. Set up API key

```bash
# Interactive login
pick --provider anthropic
# Or set directly
pick --api-key sk-ant-xxx...
```

### 2. Project configuration

`.pick/settings.json` overrides global settings `~/.pick/settings.json`:

```json
{
  "default_provider": "anthropic",
  "default_model": "claude-sonnet-4-20250514",
  "permission": {
    "approval_policy": "on_request",
    "permission_profile": ":workspace"
  }
}
```

### 3. Verify installation

```bash
pick -P "Hello, what model are you using?"
```

## Next steps

- [CLI Reference](cli.md) — Complete command-line options
- [Settings](settings.md) — Configuration reference
- [Tools](tools.md) — Built-in tools overview
- [Extensions](extensions.md) — Writing custom extensions
