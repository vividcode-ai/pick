# Custom Commands

Pick supports defining custom slash commands through Markdown files. These command files are placed in the `commands/` directory and triggered via `/command_name`.

## Command format

```markdown
---
description: Command description - short description of what the command does
argument-hint: "<argument hint>"
---

# Command Title

The specific instruction content for the command, supports Markdown.

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
| `description` | ✓ | Short description of the command (shown in help) |
| `argument-hint` | No | Argument hint text |

### Argument placeholders

Use these placeholders in command content to reference user input:

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
description: Greet the user — a friendly salutation
argument-hint: "[name]"
---

# Hello Command

When the user types /hello, respond with a friendly greeting.

## Requirements

1. Use a warm and friendly tone
2. If the user provides a name ($1), address them by name
3. Respond in English

User said: $@
```

In Pick:

```
You: /hello Alice
AI: Hello, Alice! Great to meet you! 😊
```

## Best practices

1. **Clear descriptions** — Keep `description` concise so the user knows the command's purpose
2. **Argument hints** — Provide `argument-hint` to help users understand argument format
3. **Explicit requirements** — List specific behaviors the AI should follow
4. **Use arguments** — Reference user input with `$1`, `$@`, etc.
