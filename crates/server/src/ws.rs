use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use pick_agent::core::agent_loop::AgentLoopConfig;
use pick_agent::core::state::ThinkingLevel;
use pick_ai::types::Message as AiMessage;
use pick_ai::types::UserMessage;
use tokio::sync::{Mutex, oneshot, watch};
use tracing::{error, info};

use crate::AppState;
use crate::events::{self, WsEvent};

type PendingApprovals = Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>;
type PendingQuestions =
    Arc<Mutex<HashMap<String, oneshot::Sender<Result<Vec<Vec<String>>, String>>>>>;

pub async fn handle_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<WsEvent>();

    let mut current_session_id: Option<String> = None;
    let mut current_cancel_tx: Option<watch::Sender<bool>> = None;

    let pending_approvals: PendingApprovals = Arc::new(Mutex::new(HashMap::new()));
    let pending_questions: PendingQuestions = Arc::new(Mutex::new(HashMap::new()));

    loop {
        tokio::select! {
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let value: serde_json::Value = match serde_json::from_str(&text) {
                            Ok(v) => v,
                            Err(e) => {
                                let _ = ws_sender.send(Message::Text(
                                    serde_json::json!({"type": "error", "payload": {"message": format!("Parse error: {}", e)}}).to_string().into(),
                                )).await;
                                continue;
                            }
                        };

                        let msg_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        let payload = value.get("payload");

                        match msg_type {
                            "create_session" => {
                                let model_id = payload.and_then(|p| p.get("model_id")).and_then(|v| v.as_str()).unwrap_or("claude-sonnet-4-20250514");
                                let provider = payload.and_then(|p| p.get("provider")).and_then(|v| v.as_str()).unwrap_or("anthropic");
                                let system_prompt = state.build_system_prompt(provider, model_id);
                                let tools = state.get_tools();

                                let session_id = state.session_manager.create(
                                    model_id.to_string(),
                                    provider.to_string(),
                                    system_prompt,
                                    tools,
                                ).await;

                                current_session_id = Some(session_id.clone());
                                current_cancel_tx = None;
                                let _ = ws_sender.send(Message::Text(
                                    serde_json::json!({"type": "session_created", "payload": {"session_id": session_id}}).to_string().into(),
                                )).await;
                            }
                            "ask" | "chat" | "generate" => {
                                let prompt = payload
                                    .and_then(|p| p.get("prompt"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");

                                let session_id = payload
                                    .and_then(|p| p.get("session_id"))
                                    .and_then(|v| v.as_str())
                                    .or(current_session_id.as_deref())
                                    .map(|s| s.to_string());

                                let sid = match session_id {
                                    Some(id) => id,
                                    None => {
                                        let _ = ws_sender.send(Message::Text(
                                            serde_json::json!({"type": "error", "payload": {"message": "No session. Send create_session first."}}).to_string().into(),
                                        )).await;
                                        continue;
                                    }
                                };

                                let session = state.session_manager.get(&sid).await;
                                let session = match session {
                                    Some(s) => s,
                                    None => {
                                        let _ = ws_sender.send(Message::Text(
                                            serde_json::json!({"type": "error", "payload": {"message": format!("Session {} not found", sid)}}).to_string().into(),
                                        )).await;
                                        continue;
                                    }
                                };

                                let model = match pick_ai::models::get_model(&session.provider, &session.model_id) {
                                    Some(m) => m,
                                    None => {
                                        let _ = ws_sender.send(Message::Text(
                                            serde_json::json!({"type": "error", "payload": {"message": format!("Model '{}' not found", session.model_id)}}).to_string().into(),
                                        )).await;
                                        continue;
                                    }
                                };

                                let msg = AiMessage::User(UserMessage::text(prompt));
                                let all_msgs = {
                                    let mut msgs = session.messages.clone();
                                    msgs.push(msg);
                                    msgs
                                };

                                let (cancel_tx, _cancel_rx) = watch::channel(false);
                                current_cancel_tx = Some(cancel_tx.clone());

                                let et = event_tx.clone();
                                let sid_clone = sid.clone();
                                let state_clone = state.clone();

                                let pa = pending_approvals.clone();
                                let pq = pending_questions.clone();
                                let et_pa = event_tx.clone();
                                let et_pq = event_tx.clone();

                                let approve = Some(Arc::new(move |title: String, msg_body: String| {
                                    let pa = pa.clone();
                                    let et = et_pa.clone();
                                    let approval_id = uuid::Uuid::now_v7().to_string();
                                    Box::pin(async move {
                                        let (tx, rx) = oneshot::channel();
                                        pa.lock().await.insert(approval_id.clone(), tx);
                                        let _ = et.send(WsEvent {
                                            event_type: "approval_required".to_string(),
                                            payload: serde_json::json!({
                                                "approval_id": approval_id,
                                                "tool_name": title,
                                                "tool_args": msg_body,
                                            }),
                                        });
                                        rx.await.unwrap_or(false)
                                    }) as std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
                                }) as pick_agent::core::state::ApproveFn);

                                let question = Some(Arc::new(move |questions: Vec<pick_agent::core::state::QuestionPrompt>| {
                                    let pq = pq.clone();
                                    let et = et_pq.clone();
                                    let question_id = uuid::Uuid::now_v7().to_string();
                                    Box::pin(async move {
                                        let (tx, rx) = oneshot::channel();
                                        pq.lock().await.insert(question_id.clone(), tx);
                                        let prompts: Vec<serde_json::Value> = questions.iter().map(|q| {
                                            serde_json::json!({
                                                "question": q.question,
                                                "header": q.header,
                                                "options": q.options,
                                                "multiple": q.multiple,
                                            })
                                        }).collect();
                                        let _ = et.send(WsEvent {
                                            event_type: "question".to_string(),
                                            payload: serde_json::json!({
                                                "question_id": question_id,
                                                "prompts": prompts,
                                            }),
                                        });
                                        rx.await.unwrap_or(Err("No response".to_string()))
                                    }) as std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Vec<String>>, String>> + Send>>
                                }) as pick_agent::core::state::QuestionFn);

                                let config = AgentLoopConfig {
                                    model: model.clone(),
                                    system_prompt: session.system_prompt.clone(),
                                    tools: session.tools.clone(),
                                    thinking_level: ThinkingLevel::Off,
                                    max_tokens: None,
                                    temperature: None,
                                    extension_runner: None,
                                    transform_context: None,
                                    get_api_key: None,
                                    before_tool_call: None,
                                    should_stop_after_turn: None,
                                    get_steering_messages: None,
                                    get_follow_up_messages: None,
                                    provider_max_retries: None,
                                    provider_max_retry_delay_ms: None,
                                    approve,
                                    question,
                                    agent_id: None,
                                    agent_registry: None,
                                    on_event: Some(Arc::new(move |event| {
                                        if let Some(ws_event) = events::serialize_event(&event) {
                                            let _ = et.send(ws_event);
                                        }
                                    })),
                                    fs_policy: None,
                                    cwd: None,
                                    mode_rulesets: None,
                                    permission_hooks: None,
                                    permission_manager: None,
                                    tool_event_bus: None,
                                    sandbox: None,
                                    sandbox_enabled: None,
                                    cancel_signal_tx: Some(Arc::new(cancel_tx)),
                                    skill_paths: Vec::new(),
                                    on_turn_complete: None,
                                };

                                let agent_handle = tokio::spawn(async move {
                                    pick_agent::core::agent_loop::run_agent_loop(
                                        config,
                                        all_msgs,
                                    )
                                    .await
                                });

                                let result = agent_handle.await;

                                match result {
                                    Ok(Ok(agent_result)) => {
                                        let total_input: u64 = agent_result.messages.iter()
                                            .filter_map(|m| {
                                                if let AiMessage::Assistant(a) = m {
                                                    Some(a.usage.input)
                                                } else {
                                                    None
                                                }
                                            })
                                            .sum();
                                        let total_output: u64 = agent_result.messages.iter()
                                            .filter_map(|m| {
                                                if let AiMessage::Assistant(a) = m {
                                                    Some(a.usage.output)
                                                } else {
                                                    None
                                                }
                                            })
                                            .sum();

                                        let _ = event_tx.send(events::serialize_agent_end(total_input, total_output));

                                        state_clone.session_manager.update_messages(&sid_clone, agent_result.messages).await;
                                    }
                                    Ok(Err(e)) => {
                                        let _ = event_tx.send(WsEvent {
                                            event_type: "error".to_string(),
                                            payload: serde_json::json!({"message": e}),
                                        });
                                    }
                                    Err(e) => {
                                        let _ = event_tx.send(WsEvent {
                                            event_type: "error".to_string(),
                                            payload: serde_json::json!({"message": format!("Agent task failed: {}", e)}),
                                        });
                                    }
                                }
                            }
                            "cancel" => {
                                if let Some(ref tx) = current_cancel_tx {
                                    let _ = tx.send(true);
                                }
                                let _ = ws_sender.send(Message::Text(
                                    serde_json::json!({"type": "cancelled"}).to_string().into(),
                                )).await;
                            }
                            "approve" => {
                                let approval_id = payload
                                    .and_then(|p| p.get("approval_id"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let approved = payload
                                    .and_then(|p| p.get("approved"))
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);
                                let mut map = pending_approvals.lock().await;
                                if let Some(tx) = map.remove(approval_id) {
                                    let _ = tx.send(approved);
                                }
                            }
                            "answer_question" => {
                                let question_id = payload
                                    .and_then(|p| p.get("question_id"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let answers = payload
                                    .and_then(|p| p.get("answers"))
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter().map(|a| {
                                            if let Some(arr) = a.as_array() {
                                                arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                                            } else if let Some(s) = a.as_str() {
                                                vec![s.to_string()]
                                            } else {
                                                vec![]
                                            }
                                        }).collect()
                                    })
                                    .unwrap_or_default();
                                let mut map = pending_questions.lock().await;
                                if let Some(tx) = map.remove(question_id) {
                                    let _ = tx.send(Ok(answers));
                                }
                            }
                            "ping" => {
                                let _ = ws_sender.send(Message::Text(
                                    serde_json::json!({"type": "pong"}).to_string().into(),
                                )).await;
                            }
                            _ => {
                                let _ = ws_sender.send(Message::Text(
                                    serde_json::json!({"type": "error", "payload": {"message": format!("Unknown message type: {}", msg_type)}}).to_string().into(),
                                )).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
            Some(event) = event_rx.recv() => {
                let json = serde_json::to_string(&event).unwrap_or_default();
                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    }

    info!("WebSocket connection closed");
}
