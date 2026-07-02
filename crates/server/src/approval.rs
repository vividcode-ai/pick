use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::response::sse::Event;
use pick_agent::permission::hooks::{
    HookAction, PermissionHook, PermissionRequestContext, PermissionRequestHook,
};
use tokio::sync::mpsc::UnboundedSender;

pub struct SseApprovalHook {
    pub event_tx: UnboundedSender<Result<Event, Infallible>>,
    pub pending_approvals: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
}

#[async_trait]
impl PermissionRequestHook for SseApprovalHook {
    async fn on_permission_request(&self, ctx: &PermissionRequestContext) -> HookAction {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let id = uuid::Uuid::now_v7().to_string();
        {
            self.pending_approvals
                .lock()
                .unwrap()
                .insert(id.clone(), tx);
        }
        let payload = serde_json::json!({
            "approval_id": id,
            "tool_name": ctx.tool_name,
            "tool_args": ctx.tool_args,
            "permission": ctx.permission,
            "source": "permission_hook",
        });
        let _ = self.event_tx.send(Ok(Event::default()
            .event("approval_required")
            .data(serde_json::to_string(&payload).unwrap_or_default())));
        match rx.await {
            Ok(true) => HookAction::Allow,
            _ => HookAction::Deny {
                reason: "Rejected by user".to_string(),
            },
        }
    }
}

impl PermissionHook for SseApprovalHook {
    fn name(&self) -> &str {
        "sse-approval"
    }
}
