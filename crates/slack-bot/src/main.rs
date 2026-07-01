use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use reqwest::Client;
use std::env;
use tokio_tungstenite::connect_async;
use tracing::{debug, error, info, warn};

mod slack_types;
use slack_types::*;

const SLACK_API: &str = "https://slack.com/api";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let pick_server_url =
        env::var("PICK_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let slack_bot_token = env::var("SLACK_BOT_TOKEN").context("SLACK_BOT_TOKEN is required")?;

    info!("Pick Slack Bot starting...");
    info!("Pick server URL: {}", pick_server_url);

    let http = Client::new();

    // Verify token with auth.test
    let auth_resp: AuthTestResponse = http
        .post(format!("{SLACK_API}/auth.test"))
        .header("Authorization", format!("Bearer {slack_bot_token}"))
        .send()
        .await?
        .json()
        .await?;
    if !auth_resp.ok {
        anyhow::bail!("Slack auth failed: {:?}", auth_resp.error);
    }
    info!(
        "Authenticated as bot user {} on team {}",
        auth_resp.user_id.as_deref().unwrap_or("?"),
        auth_resp.team_id.as_deref().unwrap_or("?")
    );

    // Main reconnection loop
    loop {
        if let Err(e) = run_bot(&http, &pick_server_url, &slack_bot_token).await {
            warn!("Bot session ended: {e}. Reconnecting in 5s...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }
}

async fn run_bot(http: &Client, pick_url: &str, bot_token: &str) -> Result<()> {
    // 1. Get WebSocket URL from Slack apps.connections.open
    let ws_url = get_ws_url(http, bot_token).await?;
    info!("Connecting to Slack WebSocket...");

    // 2. Connect WebSocket
    let (ws_stream, _) = connect_async(&ws_url).await?;
    let (mut write, mut read) = ws_stream.split();

    // 3. Authenticate
    let auth_msg = serde_json::to_string(&SlackAuthMessage {
        token: bot_token.to_string(),
    })?;
    write.send(auth_msg.into()).await?;
    info!("Authentication sent, waiting for events...");

    // 4. Event loop
    while let Some(msg) = read.next().await {
        let msg = msg?;
        if !msg.is_text() {
            continue;
        }
        let text = msg.to_text()?.to_string();

        let envelope: SlackEnvelope = match serde_json::from_str(&text) {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to parse envelope: {e}, raw: {text:.100}");
                continue;
            }
        };

        match envelope.msg_type.as_str() {
            "hello" => {
                info!("Slack WebSocket connected");
            }
            "disconnect" => {
                info!("Slack requested disconnect: {:?}", envelope.error);
                break;
            }
            "events_api" => {
                let envelope_id = match &envelope.envelope_id {
                    Some(id) => id.clone(),
                    None => continue,
                };

                // Acknowledge immediately
                let ack = serde_json::to_string(&SlackAck {
                    envelope_id: envelope_id.clone(),
                })?;
                let _ = write.send(ack.into()).await;

                // Process event
                if let Some(payload) = &envelope.payload
                    && let Ok(ep) = serde_json::from_value::<EventPayload>(payload.clone())
                    && let Err(e) = handle_event(http, pick_url, bot_token, &ep).await
                {
                    error!("Failed to handle event: {e}");
                }
            }
            other => {
                debug!("Unhandled message type: {other}");
            }
        }
    }

    Ok(())
}

async fn get_ws_url(http: &Client, bot_token: &str) -> Result<String> {
    let resp: AppsConnectionsOpenResponse = http
        .post(format!("{SLACK_API}/apps.connections.open"))
        .header("Authorization", format!("Bearer {bot_token}"))
        .send()
        .await?
        .json()
        .await?;
    if !resp.ok {
        anyhow::bail!(
            "apps.connections.open failed: {}",
            resp.error.unwrap_or_default()
        );
    }
    resp.url.context("No WebSocket URL returned")
}

async fn handle_event(
    http: &Client,
    pick_url: &str,
    bot_token: &str,
    payload: &EventPayload,
) -> Result<()> {
    let event = &payload.event;

    // Only handle app_mention or message in DMs (channel_type = "im")
    let is_relevant = matches!(
        (event.event_type.as_str(), event.channel_type.as_deref()),
        ("app_mention", _) | ("message", Some("im"))
    );
    if !is_relevant {
        return Ok(());
    }

    // Ignore bot's own messages and message_changed subtypes
    if event.subtype.as_deref() == Some("message_changed")
        || event.subtype.as_deref() == Some("bot_message")
    {
        return Ok(());
    }

    let text = match &event.text {
        Some(t) => t,
        None => return Ok(()),
    };

    let channel = match &event.channel {
        Some(c) => c,
        None => return Ok(()),
    };

    info!("Processing message from channel {channel}: {text:.100}");

    // Forward to pick-server /ask
    let ask_body = serde_json::json!({
        "prompt": text,
    });

    let ask_resp = match http
        .post(format!("{pick_url}/ask"))
        .json(&ask_body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("pick-server request failed: {e}");
            return Ok(());
        }
    };

    let response_text = ask_resp.text().await.unwrap_or_default();
    if response_text.is_empty() {
        warn!("Empty response from pick-server");
        return Ok(());
    }

    // Post response back to Slack
    let reply = ChatPostMessage {
        channel: channel.clone(),
        text: truncate_response(&response_text),
        thread_ts: event.thread_ts.clone().or_else(|| event.ts.clone()),
    };

    let post_resp: ChatPostMessageResponse = http
        .post(format!("{SLACK_API}/chat.postMessage"))
        .header("Authorization", format!("Bearer {bot_token}"))
        .json(&reply)
        .send()
        .await?
        .json()
        .await?;

    if !post_resp.ok {
        warn!("Failed to post message: {:?}", post_resp.error);
    }

    Ok(())
}

/// Truncate long responses to fit Slack's message limit (~40k chars)
fn truncate_response(text: &str) -> String {
    const MAX: usize = 39_000;
    if text.len() <= MAX {
        text.to_string()
    } else {
        format!("{}...\n\n_(Response truncated)_", &text[..MAX])
    }
}
