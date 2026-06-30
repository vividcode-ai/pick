# Session Management

Sessions persist as JSONL files, supporting resume, fork, compaction, and navigation.

## Storage format

One JSON object per line at:

```
~/.pick/agent/sessions/
  ├── <session-id>.jsonl
  └── ...
```

Each record is a `SessionEntry` containing messages, tool calls, and system events.

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
pick                   # Auto-create new session
pick --no-session      # Run without persistence
```

### Resume

```bash
pick -s <session-id>   # Resume by ID
pick -r                # Interactive session selector
pick -c                # Continue most recent session
```

### Fork

```bash
pick --fork <session-id>   # Snapshot then continue
```

## Session compaction

Long sessions trigger compaction to reduce context length:

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
| `max_messages` | 150 | Max messages before compaction |
| `max_tokens` | 32000 | Max tokens before compaction |
| `enabled` | true | Enable automatic compaction |

Extensions can customize via `session_before_compact` event.

## Storage modes

| Mode | Description |
|------|-------------|
| **Persistent** (default) | JSONL file, supports resume and fork |
| **In-memory** | Memory-only, lost on process end |

## State persistence

Extension tool state persists via the `details` field in tool results. On resume, extensions reconstruct state by iterating branch entries.

## Export

```bash
pick --export session.html   # Export most recent session
```

## Session titles

Default format: `New session - <timestamp>`.
