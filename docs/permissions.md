# Permission System

Pick has a three-tier permission system ensuring AI operations stay within controlled bounds.

## Architecture

```
┌──────────────┐     ┌───────────────┐     ┌──────────────┐
│  Tool Call   │────▶│  Permission   │────▶│  Execution   │
│  Request     │     │  Evaluation   │     │  Decision    │
└──────────────┘     └───────────────┘     └──────────────┘
                           │
                    ┌──────┴──────┐
                    │              │
               ┌─────────┐  ┌──────────┐
               │ Ruleset │  │  Audit   │
               │         │  │  Log     │
               └─────────┘  └──────────┘
```

## Permission keys

Each tool maps to a permission key:

| Tool | Permission key |
|------|----------------|
| `read` | `read` |
| `write`, `edit`, `apply_patch`, `multiedit` | `edit` |
| `bash` | `bash` |
| `grep` | `grep` |
| `glob`, `find` | `glob` |
| `ls` | `list` |
| `subagent`, `task` | `subagent` |
| `question` | `question` |
| `webfetch` | `webfetch` |
| `todo_plan` | `todo_plan` |
| `plan_enter` / `plan_exit` | `plan_enter` / `plan_exit` |

## Rule evaluation

Rule config file (`.pick/rules/default.rules`):

```text
# This is a comment

# Safe commands
ls -> allow
cat -> allow
head -> allow
git diff -> allow

# Dangerous commands - prompt for confirmation
rm -rf -> prompt
sudo -> prompt
git push -> prompt

# Forbidden commands
rm -rf / -> forbid
dd -> forbid
```

### Syntax

One rule per line: `<pattern> -> <decision>`

### Decisions

| Decision | Description |
|----------|-------------|
| `allow` | Auto-allow |
| `prompt` | Ask user for confirmation |
| `forbid` / `forbidden` | Directly deny |

### Pattern matching

- Token-aware pattern matching (`git push` matches `git push origin main`)
- Wildcard support (`git *` matches any git subcommand)
- **Last matching rule wins**
- Commands containing shell meta characters (`;`, `&&`, `||`, `|`, `$()`) are auto-upgraded to `prompt`

### Built-in security heuristics

When no rule matches, built-in heuristics apply:

| Category | Example commands | Default decision |
|----------|-----------------|------------------|
| Safe | ls, cat, head, tail, grep, find, echo, pwd | `allow` |
| Dangerous | rm, mv, sudo, chmod, kill, dd, mkfs | `prompt` |
| Unknown | Other commands | `prompt` |

## Permission rules

Permission rule files (JSON format) define more granular access control:

```json
[
  {
    "permission": "edit",
    "pattern": "*.md",
    "action": "allow"
  },
  {
    "permission": "edit",
    "pattern": ".env",
    "action": "deny"
  },
  {
    "permission": "bash",
    "pattern": "git push",
    "action": "ask"
  }
]
```

Wildcard matching is supported for both permission keys and patterns.

### Save and load

- Rule files are stored at `~/.pick/permissions.json` (global) and `.pick/permissions.json` (project)
- Project rules override global rules
- Can be modified at runtime via the `/permission` command

## Audit log

Every permission decision is recorded in the audit log:

```jsonl
{"type":"allow","tool":"bash","command":"ls -la","timestamp":"...","layer":"base"}
{"type":"ask","tool":"edit","file":"/etc/hosts","timestamp":"...","layer":"base"}
{"type":"deny","tool":"bash","command":"sudo rm -rf /","timestamp":"...","layer":"base"}
```

### Viewing audit

```bash
# View all audit entries
pick --audit

# Filter by tool
pick --audit --tool bash

# Filter by decision
pick --audit --decision deny

# JSON output
pick --audit --json

# Last 20 entries
pick --audit --recent 20
```

## Security profiles

| Profile | Description |
|---------|-------------|
| `:workspace` | Restrict tools to workspace directory only |
| `:global` | Allow global operations (requires confirmation) |
| Custom path | Restrict to a specific directory range |

## Best practices

1. **Principle of least privilege** — Only allow the tools and operations necessary for the task
2. **Regular auditing** — Use `--audit` to review the AI's behavior records
3. **Project isolation** — Set different permission configs for different projects
4. **Periodic cleanup** — Review and update rule files regularly
5. **Sensitive path protection** — Explicitly deny write access to `.env`, `.git/`, `node_modules/`, etc.
