# Extension System

Pick extensions are Rust dynamic libraries (`.dll` / `.so` / `.dylib`) that inject custom behavior via `libloading`.

## Architecture

```
Agent Loop ──▶ ExtensionRunner ──▶ Extension Pool
```

## ExtensionFactory trait

```rust
#[async_trait]
pub trait ExtensionFactory: Send + Sync {
    fn name(&self) -> &str;
    async fn init(&self, api: &dyn ExtensionAPI) -> Result<(), String>;
}
```

## ExtensionAPI

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

Blockable events control flow by returning `EventResult`:

```rust
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

Extensions load from (in order):
1. `~/.pick/extensions/*.so` (auto-discovered)
2. `.pick/extensions/*.so` (auto-discovered)
3. `pick -e /path/to/extension.so` (CLI argument)

## Writing an extension

```rust
// my-extension/src/lib.rs
use pick_agent::extensions::{ExtensionAPI, ExtensionFactory, ExtensionEvent, EventResult};

struct MyExtension;

#[async_trait::async_trait]
impl ExtensionFactory for MyExtension {
    fn name(&self) -> &str { "my-extension" }

    async fn init(&self, api: &dyn ExtensionAPI) -> Result<(), String> {
        api.on_raw("tool_call", std::sync::Arc::new(|event| Ok(None)))?;
        api.register_tool(ToolDefinition {
            name: "my_tool".into(),
            label: "My Custom Tool".into(),
            description: "Does something useful".into(),
            ..Default::default()
        })?;
        api.register_command("my-command", Some("Description"))?;
        Ok(())
    }
}

#[no_mangle]
pub extern "C" fn _pick_extension_register() {
    pick_agent::extensions::loader::register_extension_factory(
        std::sync::Arc::new(MyExtension)
    );
}
```

## Differences from pi

| Feature | pi | Pick |
|---------|----|------|
| Language | TypeScript | Rust |
| Loading | jiti dynamic load | libloading shared library |
| Event API | `pi.on("name", handler)` | `on_raw("name", handler)` |
| Tool registration | `pi.registerTool({...})` | `register_tool(ToolDefinition{...})` |
| State persistence | Auto via `details` field | Manual management |
