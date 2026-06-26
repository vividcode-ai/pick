# Extension System

Pick provides an extension mechanism via dynamic library loading (libloading). Extensions inject custom behavior at various stages of the agent lifecycle.

> **Note:** Pick's extension system is based on Rust dynamic libraries, unlike pi (TypeScript). Extensions must be compiled as `.dll` / `.so` / `.dylib` shared libraries.

## Architecture

```
┌──────────────┐     ┌──────────────────┐
│  Agent Loop  │────▶│  ExtensionRunner │
└──────────────┘     └──────────────────┘
     │                      │
     │  Event               │  Dispatch to
     │  Emission            │  registered handlers
     ▼                      ▼
┌────────────────────────────────────────────┐
│              Extension Pool                │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐      │
│  │ Ext #1  │ │ Ext #2  │ │ Ext #3  │ ...  │
│  └─────────┘ └─────────┘ └─────────┘      │
└────────────────────────────────────────────┘
```

## ExtensionFactory trait

Extensions implement the `ExtensionFactory` trait:

```rust
#[async_trait]
pub trait ExtensionFactory: Send + Sync {
    fn name(&self) -> &str;
    async fn init(&self, api: &dyn ExtensionAPI) -> Result<(), String>;
}
```

## ExtensionAPI

The `ExtensionAPI` trait provides these registration methods:

| Method | Description |
|--------|-------------|
| `on_raw(event_type, handler)` | Register an event handler |
| `register_tool(tool)` | Register a custom tool |
| `register_command(name, description)` | Register a slash command |
| `register_shortcut(shortcut, description)` | Register a keyboard shortcut |
| `register_flag(name, description, type, default)` | Register a CLI flag |

## Lifecycle events

| Event | Trigger | Blockable |
|-------|---------|-----------|
| `resources_discover` | Resource discovery (startup/reload) | — |
| `session_start` | Session start, resume, fork | — |
| `session_before_switch` | Before session switch | ✓ |
| `session_before_fork` | Before session fork | ✓ |
| `session_shutdown` | Session shutdown | — |
| `context` | Context building | — |
| `before_provider_request` | Before LLM request | — |
| `after_provider_response` | After LLM response | — |
| `before_agent_start` | Before agent loop starts | — |
| `agent_start` | Agent loop starts | — |
| `agent_end` | Agent loop ends | — |
| `turn_start` | Single turn starts | — |
| `turn_end` | Single turn ends | — |
| `message_start` | Message streaming starts | — |
| `message_update` | Message delta update | — |
| `message_end` | Message streaming completes | — |
| `tool_call` | Tool is invoked | ✓ |
| `tool_result` | Tool execution completes | — |
| `user_bash` | User executes !command | — |
| `input` | User input | — |
| `session_before_compact` | Before session compaction | — |
| `session_compact` | After session compaction | — |
| `session_before_tree` | Before session tree operation | — |
| `session_tree` | After session tree operation | — |

**Blockable events** control flow by returning a result:

```rust
// tool_call event example: blocking dangerous commands
fn my_handler(event: &ExtensionEvent) -> EventResult {
    if let ExtensionEvent::ToolCall(ev) = event {
        if ev.tool_name == "bash" && ev.input["command"].to_string().contains("rm -rf") {
            return Err("Dangerous command blocked".to_string());
        }
    }
    Ok(None)
}
```

## Extension loading

Extensions are loaded from these locations:

1. **User directory** — `~/.pick/extensions/*.so` (auto-discovered)
2. **Project directory** — `.pick/extensions/*.so` (auto-discovered)
3. **CLI argument** — `pick -e /path/to/extension.so`

## Writing an extension

An extension is a Rust crate that implements the `ExtensionFactory` trait:

```rust
// my-extension/src/lib.rs
use pick_agent::extensions::{
    ExtensionAPI, ExtensionFactory, ExtensionEvent, EventResult,
};

struct MyExtension;

#[async_trait::async_trait]
impl ExtensionFactory for MyExtension {
    fn name(&self) -> &str {
        "my-extension"
    }

    async fn init(&self, api: &dyn ExtensionAPI) -> Result<(), String> {
        // Register event handler
        api.on_raw("tool_call", std::sync::Arc::new(|event| {
            // Handle event
            Ok(None)
        }));

        // Register custom tool
        api.register_tool(ToolDefinition {
            name: "my_tool".into(),
            label: "My Custom Tool".into(),
            description: "Does something useful".into(),
            parameters: vec![],
            prompt_snippet: None,
            prompt_guidelines: None,
            render_shell: None,
            execution_mode: None,
        });

        // Register slash command
        api.register_command("my-command", Some("Description"));

        Ok(())
    }
}

// Register globally
#[no_mangle]
pub extern "C" fn _pick_extension_register() {
    pick_agent::extensions::loader::register_extension_factory(
        std::sync::Arc::new(MyExtension)
    );
}
```

## Differences from pi extensions

| Feature | pi | Pick |
|---------|----|------|
| Language | TypeScript | Rust |
| Loading | jiti dynamic load | libloading shared library |
| Event API | `pi.on("name", handler)` | `on_raw("name", handler)` |
| Tool registration | `pi.registerTool({...})` | `register_tool(ToolDefinition{...})` |
| State persistence | Auto via `details` field | Manual management |
