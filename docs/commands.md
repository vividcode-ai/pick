# Custom Commands

Pick supports custom slash commands via Markdown files in the `commands/` directory, triggered as `/command_name`.

## Command format

```markdown
---
description: Short description of what the command does
argument-hint: "<argument hint>"
---

# Command Title

Instructions for the command, supports Markdown.

## Requirements

1. Specific requirements for the AI to follow
2. Each requirement affects the AI's behavior

## Arguments

- `$1` - First argument
- `$2` - Second argument
- `$@` - All arguments
```

### Frontmatter fields

| Field | Required | Description |
|-------|----------|-------------|
| `description` | ✓ | Short description (shown in help) |
| `argument-hint` | No | Argument hint text |

### Argument placeholders

| Placeholder | Description |
|-------------|-------------|
| `$1`, `$2`, ... | Nth argument |
| `$@` | All arguments |
| `$0` | Full command name |

## Load locations

Commands are discovered from these locations (lowest to highest priority):

| Location | Description |
|----------|-------------|
| `~/.pick/agent/commands/<name>.md` | Global user commands |
| `.pick/commands/<name>.md` | Project commands |
| `--skill <path>` | Commands registered via skills |

## Usage example

Create `.pick/commands/hello.md`:

```markdown
---
description: Greet the user
argument-hint: "[name]"
---

Greet the user warmly. If a name ($1) is provided, address them by name. Respond in English.

User said: $@
```

```
You: /hello Alice
AI: Hello, Alice! Great to meet you!
```

## Best practices

1. **Clear descriptions** — Keep `description` concise so the user understands the command's purpose
2. **Argument hints** — Provide `argument-hint` to guide argument format
3. **Explicit requirements** — List specific behaviors the AI should follow
4. **Use arguments** — Reference user input with `$1`, `$@`, etc.
