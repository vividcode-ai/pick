# Sub-agents

Sub-agents allow task delegation to dedicated agent processes with isolated context windows. Each sub-agent has its own system prompt, tool set, and model configuration.

## Agent definition

Agents are Markdown files with YAML frontmatter:

```markdown
---
name: scout
description: Fast codebase reconnaissance — returns structured context summary
tools: read, grep, find, ls, bash
model: claude-haiku-4-5
---

You are a code scout. Your task is to quickly scan the codebase and extract key information.

## Behavior

1. Only use read, grep, find, ls, and bash tools
2. Output must be structured Markdown
3. Focus on core logic and data flow, skip implementation details
```

### Frontmatter fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | ✓ | Agent name |
| `description` | ✓ | Agent description (shown to users and LLM) |
| `tools` | No | Tool allowlist (comma-separated, defaults to all built-in tools) |
| `model` | No | Model to use (defaults to parent agent's model) |

## Load locations

| Location | Scope | Trust |
|----------|-------|-------|
| `~/.pick/agent/agents/*.md` | User-level (always loaded) | Trusted |
| `.pick/agents/*.md` (requires `agentScope` config) | Project-level | Requires confirmation |

### Security model

Project-level agents (`.pick/agents/*.md`) are repo-controlled and can read files and execute commands.
**Only user-level agents are loaded by default.** Enable project-level agents by setting `agentScope: "project"` or `"both"`,
and exercise caution with untrusted repositories.

## Usage patterns

### Single agent

```
Use scout to find all authentication-related code
```

### Parallel execution

```
Run 2 scouts in parallel: one to find model definitions, one to find API routes
```

### Chained workflow

```
Use a chain: first have scout find the auth module, then have planner suggest refactoring
```

### Workflow prompts

If workflow prompt templates are configured:

```
/implement Add Redis caching to the API
/scout-and-plan Refactor auth module to support OAuth
```

## Example agents

### Scout (fast scan)

```markdown
---
name: scout
description: Fast codebase reconnaissance
tools: read, grep, find, ls, bash
model: claude-haiku-4-5
---

Quickly scan the codebase and return a structured context summary.
```

### Planner (design)

```markdown
---
name: planner
description: Implementation plan design
tools: read, grep, find, ls
model: claude-sonnet-4-20250514
---

Analyze the problem and create an implementation plan. Do not modify files.
```

### Reviewer (code review)

```markdown
---
name: reviewer
description: Code review
tools: read, grep, find, ls, bash
model: claude-sonnet-4-20250514
---

Review code changes for correctness, performance, and security.
```

### Worker (general execution)

```markdown
---
name: worker
description: General-purpose execution agent
---

Execute assigned tasks with full tool capabilities.
```

## Output display

**Collapsed view (default):**
- Status icon (✓/✗/⏳) and agent name
- Last 5-10 operation records
- Usage stats: turns, tokens, cost

**Expanded view (Ctrl+O):**
- Full task description
- All tool calls and arguments
- Final output (Markdown rendered)
- Per-step statistics

## Limitations

- Parallel mode: max 8 tasks, 4 concurrent
- Task output capped at 50 KB (expand to see full results)
- Agents are re-discovered on each invocation (edits take effect immediately)
