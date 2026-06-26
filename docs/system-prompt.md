# System Prompt

Pick supports customizing and appending to the system prompt via files, allowing you to adjust the AI's behavior.

## SYSTEM.md

`SYSTEM.md` **completely replaces** the default system prompt.

### Load locations

| Location | Priority |
|----------|----------|
| `--system-prompt "..."` CLI argument | Highest |
| `.pick/SYSTEM.md` | Project-level |
| `~/.pick/agent/SYSTEM.md` | User-level |

### Example

```markdown
You are an AI coding assistant running inside Pick.

# Core Behavior

- Be concise and direct
- Read and understand code before making changes
- Always consider edge cases and error handling
- Write tests for new functionality

# Communication Style

- Use plain text for explanations
- Use fenced code blocks for code samples
- Show file paths clearly when referencing files
```

## APPEND_SYSTEM.md

`APPEND_SYSTEM.md` appends content to the **end** of the default system prompt.

### Load locations

| Location | Priority |
|----------|----------|
| `.pick/APPEND_SYSTEM.md` | Project-level |
| `~/.pick/agent/APPEND_SYSTEM.md` | User-level |

### Example

```markdown
# Project-Specific Rules

This project follows these conventions:

1. Use Rust edition 2024
2. Prefer anyhow for error handling
3. All public APIs must have doc comments
4. All configuration uses JSON format
```

## Override rules

```text
[Default system prompt]
    + APPEND_SYSTEM.md (global → project → appended)
    OR
[SYSTEM.md (full replacement)]
    + APPEND_SYSTEM.md (still appended)
```

1. **No SYSTEM.md** → Default system prompt is used
2. **SYSTEM.md exists** → Replaces default system prompt
3. **APPEND_SYSTEM.md** → Always appended to the final system prompt

## Dynamic appending

Extensions can use `systemPromptAppend` to append content to the system prompt at runtime.

## Best practices

1. **Prefer APPEND_SYSTEM.md** — Append rules rather than fully replacing
2. **Use SYSTEM.md for full customization** — Only replace the default prompt when you need full control
3. **Place project config in .pick/** — Keep projects self-contained
4. **Place global config in ~/.pick/** — For cross-project rules
5. **Avoid over-constraining** — Too many rules reduce AI flexibility
