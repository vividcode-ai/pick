use std::sync::Arc;

use async_trait::async_trait;

#[derive(Debug, Clone)]
pub enum HookAction {
    Allow,
    Deny { reason: String },
    Continue,
}

#[derive(Debug, Clone)]
pub struct PreToolUseContext {
    pub tool_name: String,
    pub tool_call_id: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct PostToolUseContext {
    pub tool_name: String,
    pub tool_call_id: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub is_error: bool,
}

#[derive(Debug, Clone)]
pub struct PermissionRequestContext {
    pub tool_name: String,
    pub tool_args: String,
    pub permission: String,
    pub reason: Option<String>,
}

pub trait PermissionHook: Send + Sync {
    fn name(&self) -> &str;
    fn priority(&self) -> u8 {
        0
    }
}

pub trait PreToolUseHook: PermissionHook {
    fn on_pre_tool_use(&self, ctx: &PreToolUseContext) -> HookAction;
}

pub trait PostToolUseHook: PermissionHook {
    fn on_post_tool_use(&self, ctx: &PostToolUseContext);
}

#[async_trait]
pub trait PermissionRequestHook: PermissionHook {
    async fn on_permission_request(&self, ctx: &PermissionRequestContext) -> HookAction;
}

type BoxedPreHook = Arc<dyn PreToolUseHook>;
type BoxedPostHook = Arc<dyn PostToolUseHook>;
type BoxedPermissionHook = Arc<dyn PermissionRequestHook>;

pub struct PermissionHookRegistry {
    pre_hooks: std::sync::Mutex<Vec<BoxedPreHook>>,
    post_hooks: std::sync::Mutex<Vec<BoxedPostHook>>,
    permission_hooks: std::sync::Mutex<Vec<BoxedPermissionHook>>,
}

impl PermissionHookRegistry {
    pub fn new() -> Self {
        Self {
            pre_hooks: std::sync::Mutex::new(Vec::new()),
            post_hooks: std::sync::Mutex::new(Vec::new()),
            permission_hooks: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn register_pre_hook(&self, hook: BoxedPreHook) {
        if let Ok(mut hooks) = self.pre_hooks.lock() {
            hooks.push(hook);
            hooks.sort_by_key(|h| std::cmp::Reverse(h.priority()));
        }
    }

    pub fn register_post_hook(&self, hook: BoxedPostHook) {
        if let Ok(mut hooks) = self.post_hooks.lock() {
            hooks.push(hook);
            hooks.sort_by_key(|h| std::cmp::Reverse(h.priority()));
        }
    }

    pub fn register_permission_hook(&self, hook: BoxedPermissionHook) {
        if let Ok(mut hooks) = self.permission_hooks.lock() {
            hooks.push(hook);
            hooks.sort_by_key(|h| std::cmp::Reverse(h.priority()));
        }
    }

    pub fn run_pre_hooks(&self, ctx: &PreToolUseContext) -> Option<String> {
        let hooks = self.pre_hooks.lock().ok()?;
        for hook in hooks.iter() {
            match hook.on_pre_tool_use(ctx) {
                HookAction::Deny { reason } => return Some(reason),
                HookAction::Allow => return None,
                HookAction::Continue => {}
            }
        }
        None
    }

    pub fn run_post_hooks(&self, ctx: &PostToolUseContext) {
        if let Ok(hooks) = self.post_hooks.lock() {
            for hook in hooks.iter() {
                hook.on_post_tool_use(ctx);
            }
        }
    }

    pub async fn run_permission_hooks(&self, ctx: &PermissionRequestContext) -> Option<bool> {
        let hook_refs = {
            let hooks = self.permission_hooks.lock().ok()?;
            hooks.iter().map(|h| Arc::clone(h)).collect::<Vec<_>>()
        };
        for hook in hook_refs.iter() {
            match hook.on_permission_request(ctx).await {
                HookAction::Allow => return Some(true),
                HookAction::Deny { .. } => return Some(false),
                HookAction::Continue => {}
            }
        }
        None
    }

    pub fn has_pre_hooks(&self) -> bool {
        self.pre_hooks
            .lock()
            .map(|h| !h.is_empty())
            .unwrap_or(false)
    }

    pub fn has_post_hooks(&self) -> bool {
        self.post_hooks
            .lock()
            .map(|h| !h.is_empty())
            .unwrap_or(false)
    }

    pub fn has_permission_hooks(&self) -> bool {
        self.permission_hooks
            .lock()
            .map(|h| !h.is_empty())
            .unwrap_or(false)
    }
}

/// Simple CLI approval hook that asks the user via stdin
pub struct CliApprovalHook {
    pub name: String,
}

impl CliApprovalHook {
    pub fn new() -> Self {
        Self {
            name: "cli-approval".to_string(),
        }
    }
}

impl PermissionHook for CliApprovalHook {
    fn name(&self) -> &str {
        &self.name
    }
}

#[async_trait]
impl PermissionRequestHook for CliApprovalHook {
    async fn on_permission_request(&self, ctx: &PermissionRequestContext) -> HookAction {
        let reason = ctx.reason.clone();
        let tool_name = ctx.tool_name.clone();
        let tool_args = ctx.tool_args.clone();

        tokio::task::spawn_blocking(move || {
            use std::io::Write;

            let reason = reason.as_deref().unwrap_or("No reason provided");
            eprintln!(
                "\n[Permission Request] Tool '{0}' with args '{1}' requires approval.",
                tool_name, tool_args
            );
            eprintln!("  Reason: {reason}");
            eprint!("  Approve? [y/N] ");
            std::io::stdout().flush().ok();

            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            let trimmed = input.trim().to_lowercase();

            if trimmed == "y" || trimmed == "yes" {
                HookAction::Allow
            } else {
                HookAction::Deny {
                    reason: "Rejected by user".to_string(),
                }
            }
        })
        .await
        .unwrap_or(HookAction::Deny {
            reason: "Approval prompt cancelled".to_string(),
        })
    }
}
