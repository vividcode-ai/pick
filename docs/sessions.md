# Session Management

Pick persists sessions as JSONL files, supporting resume, fork, compaction, and navigation.

## Storage format

Sessions are stored as JSONL (one JSON object per line) at:

```
~/.pick/agent/sessions/
  ├── <session-id>.jsonl
  └── ...
```

Each record is a session entry (`SessionEntry`) containing messages, tool calls, system events, etc.

## Lifecycle

```text
Create ─▶ Session Start ─▶ Agent Loop ─▶ Save ─▶ Close
                              │
                         ┌────┴────┐
                         │         │
                     Resume      Fork
```

### Create

```bash
# Auto-create new session
pick

# Run without persistence
pick --no-session
```

### Resume

```bash
# Resume by ID
pick -s <session-id>

# Interactive session selector
pick -r

# Continue most recent session
pick -c
```

### Fork

```bash
# Fork from a specific session (snapshot then continue)
pick --fork <session-id>
```

## Session compaction

Long sessions trigger compaction to reduce context length. Compaction behavior is configurable:

```json
{
  "compaction": {
    "max_messages": 150,
    "max_tokens": 32000,
    "enabled": true
  }
}
```

| Setting | Default | Description |
|---------|---------|-------------|
| `max_messages` | 150 | Max messages before compaction triggers |
| `max_tokens` | 32000 | Max tokens before compaction triggers |
| `enabled` | true | Enable automatic compaction |

Extensions can customize compaction behavior via the `session_before_compact` event.

## Storage modes

| Mode | Description |
|------|-------------|
| **Persistent** (default) | Stored to JSONL file, supports resume and fork |
| **In-memory** | Memory-only storage, lost when process ends |

## State persistence

Extension tool state is persisted to sessions via the `details` field in tool results. On session resume, extensions reconstruct state by iterating through branch entries.

## Export

```bash
# Export most recent session to HTML
pick --export session.html
```

## Session titles

Default title format is `New session - <timestamp>`.
