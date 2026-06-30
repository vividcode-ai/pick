# Sub-agents

Sub-agents delegate tasks to dedicated agent processes with isolated context windows. Each sub-agent has its own system prompt, tool set, and model configuration.

## Agent definition

Agents are Markdown files with YAML frontmatter:

```markdown
---
name: scout
description: Fast codebase reconnaissance — returns structured context summary
tools: read, grep, find, ls, bash
model: claude-haiku-4-5
---

Quickly scan the codebase and extract key information.
```

### Frontmatter fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | ✓ | Agent name |
| `description` | ✓ | Description shown to users and LLM |
| `tools` | No | Comma-separated tool allowlist (defaults to all built-in tools) |
| `model` | No | Model override (defaults to parent agent's model) |

## Load locations

| Location | Scope | Trust |
|----------|-------|-------|
| `~/.pick/agent/agents/*.md` | User-level (always loaded) | Trusted |
| `.pick/agents/*.md` (requires `agentScope` config) | Project-level | Requires confirmation |

Project-level agents are repo-controlled and can execute commands. Only user-level agents load by default. Set `agentScope: "project"` or `"both"` to enable project agents.

## Usage patterns

```
# Single agent
Use scout to find all authentication-related code

# Parallel execution
Run 2 scouts in parallel: one for model definitions, one for API routes

# Chained workflow
Use a chain: first have scout find the auth module, then planner suggest refactoring

# Workflow prompts (when configured)
/implement Add Redis caching to the API
/scout-and-plan Refactor auth module to support OAuth
```

## Output display

**Collapsed view (default):** Status icon (✓/✗/⏳) and name, last 5-10 operations, usage stats (turns, tokens, cost).

**Expanded view (Ctrl+O):** Full task description, tool calls, final output (Markdown rendered), per-step statistics.

## Limitations

- Parallel mode: max 8 tasks, 4 concurrent
- Task output capped at 50 KB (expandable)
- Agents re-discovered on each invocation (edits take effect immediately)
