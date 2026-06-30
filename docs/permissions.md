# Permission System

Three-tier permission system ensuring AI operations stay within controlled bounds.

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
# Safe commands
ls -> allow
cat -> allow
head -> allow
git diff -> allow

# Dangerous — prompt for confirmation
rm -rf -> prompt
sudo -> prompt
git push -> prompt

# Forbidden
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

- Token-aware matching (`git push` matches `git push origin main`)
- Wildcard (`git *` matches any git subcommand)
- **Last matching rule wins**
- Commands with shell meta chars (`;`, `&&`, `||`, `|`, `$()`) auto-upgrade to `prompt`

### Built-in heuristics (when no rule matches)

| Category | Examples | Default |
|----------|----------|---------|
| Safe | ls, cat, head, tail, grep, find, echo, pwd | `allow` |
| Dangerous | rm, mv, sudo, chmod, kill, dd, mkfs | `prompt` |
| Unknown | Other commands | `prompt` |

## Permission rules (JSON)

Granular access control via rule files:

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

- Stored at `~/.pick/permissions.json` (global) and `.pick/permissions.json` (project)
- Project rules override global rules
- Modify at runtime via `/permission` command

## Audit log

Every decision is recorded:

```jsonl
{"type":"allow","tool":"bash","command":"ls -la","timestamp":"...","layer":"base"}
{"type":"ask","tool":"edit","file":"/etc/hosts","timestamp":"...","layer":"base"}
{"type":"deny","tool":"bash","command":"sudo rm -rf /","timestamp":"...","layer":"base"}
```

### Viewing

```bash
pick --audit                          # All entries
pick --audit --tool bash              # Filter by tool
pick --audit --decision deny          # Filter by decision
pick --audit --json                   # JSON output
pick --audit --recent 20             # Last 20 entries
```

## Security profiles

| Profile | Description |
|---------|-------------|
| `:workspace` | Restrict to workspace directory |
| `:global` | Allow global operations (requires confirmation) |
| Custom path | Restrict to a specific directory |

## Best practices

1. **Least privilege** — Allow only necessary tools and operations
2. **Regular auditing** — Use `--audit` to review AI behavior
3. **Project isolation** — Different permission configs per project
4. **Periodic cleanup** — Review and update rule files
5. **Sensitive paths** — Deny write access to `.env`, `.git/`, `node_modules/`, etc.
