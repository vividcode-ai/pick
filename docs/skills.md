# Skills System

Skills are reusable Markdown files with instructions for specific tasks. When a task matches a skill's description, Pick loads it as a supplement to the system prompt.

## Skill format

Markdown with optional YAML frontmatter:

```markdown
---
name: my-skill
description: Use this skill when the user asks about CI configuration
disable-model-invocation: false
---

# CI/CD Configuration Guide

When the user needs to configure CI/CD:

1. First check the project root for `.github/workflows/`
2. Provide a standard Rust build template
3. Ensure caching is configured correctly
```

### Frontmatter fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | ✓ | Lowercase alphanumeric + hyphens, max 64 chars |
| `description` | ✓ | Description, max 1024 chars |
| `disable-model-invocation` | No | If `true`, model won't auto-invoke (default `false`) |

### Name rules

- Only `a-z`, `0-9`, and hyphens (`-`)
- Cannot start or end with hyphen
- No consecutive double hyphens

## Load locations

| Location | Scope | Description |
|----------|-------|-------------|
| `~/.pick/agent/skills/` | User/global | Cross-project skills |
| `.pick/skills/` | Project | Project-specific skills |
| `--skill <PATH>` | CLI | File or directory path |

## Discovery

```
~/.pick/agent/skills/             # User skills
  ├── review-rust.md
  └── docker/
      └── SKILL.md                # SKILL.md in subdirectory

.pick/skills/                     # Project skills
  └── project-conventions.md
```

**Rules:**
- Directory with `SKILL.md` → only that file loads
- Otherwise, all `.md` files in directory scan
- Subdirectories recursed for `SKILL.md` files
- Duplicate names: project overrides user

## Format in system prompt

Loaded skills inject as XML blocks:

```xml
<available_skills>
  <skill>
    <name>review-rust</name>
    <description>Review Rust code for common issues</description>
    <location>/home/user/.pick/agent/skills/review-rust.md</location>
  </skill>
</available_skills>
```

## Usage example

Create `.pick/skills/debug-rust.md`:

```markdown
---
name: debug-rust
description: Help users debug Rust compilation errors and runtime issues
---

# Rust Debugging Guide

When debugging Rust code:

1. Run `cargo check` first for precise error information
2. Check borrow checker errors — the most common Rust compiler errors
3. For `unwrap()` panics, recommend `match` or `if let` instead
4. For async code, verify `.await` is in the correct context
5. Check `Cargo.toml` for dependency version conflicts
```

The skill auto-loads when the user asks about Rust compilation errors.

## Disabling model invocation

For skills intended for manual invocation via `/skill`, not automatic triggering:

```markdown
---
name: manual-skill
description: Manual invocation only
disable-model-invocation: true
---
```
