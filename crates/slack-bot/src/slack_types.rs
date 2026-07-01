use serde::{Deserialize, Serialize};

/// Response from Slack `apps.connections.open` API
#[derive(Debug, Deserialize)]
pub struct AppsConnectionsOpenResponse {
    pub ok: bool,
    pub url: Option<String>,
    pub error: Option<String>,
}

/// Message sent to Slack WebSocket to authenticate
#[derive(Debug, Serialize)]
pub struct SlackAuthMessage {
    pub token: String,
}

/// Envelope-level message from Slack Socket Mode
#[derive(Debug, Deserialize)]
pub struct SlackEnvelope {
    #[serde(rename = "envelope_id")]
    pub envelope_id: Option<String>,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub payload: Option<serde_json::Value>,
    #[expect(dead_code)]
    pub accepts_response_payload: Option<bool>,
    #[expect(dead_code)]
    pub retry_attempt: Option<u32>,
    pub error: Option<String>,
}

/// Acknowledgement sent back to Slack for an event
#[derive(Debug, Serialize)]
pub struct SlackAck {
    #[serde(rename = "envelope_id")]
    pub envelope_id: String,
}

/// Event payload from Slack
#[derive(Debug, Deserialize)]
pub struct EventPayload {
    pub event: SlackEvent,
    #[expect(dead_code)]
    pub event_id: Option<String>,
    #[expect(dead_code)]
    pub event_time: Option<u64>,
}

/// A Slack event
#[derive(Debug, Deserialize)]
pub struct SlackEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub text: Option<String>,
    pub channel: Option<String>,
    #[expect(dead_code)]
    pub user: Option<String>,
    pub thread_ts: Option<String>,
    pub ts: Option<String>,
    pub channel_type: Option<String>,
    pub subtype: Option<String>,
}

/// Message to post to Slack via Web API
#[derive(Debug, Serialize)]
pub struct ChatPostMessage {
    pub channel: String,
    pub text: String,
    pub thread_ts: Option<String>,
}

/// Response from Slack `chat.postMessage` API
#[derive(Debug, Deserialize)]
pub struct ChatPostMessageResponse {
    pub ok: bool,
    pub error: Option<String>,
}

/// Response from Slack `auth.test` API
#[derive(Debug, Deserialize)]
pub struct AuthTestResponse {
    pub ok: bool,
    pub user_id: Option<String>,
    pub team_id: Option<String>,
    pub error: Option<String>,
}
