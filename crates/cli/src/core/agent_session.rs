use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

use pick_agent::core::agent_loop::{run_agent_loop, run_agent_loop_continue, AgentLoopConfig, AgentRunResult};
use pick_agent::core::events::AgentEvent;
use pick_ai::types::Message;

use super::settings::RetrySettings;

/// Retry configuration for agent session-level retry
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub enabled: bool,
    pub max_retries: u32,
    pub base_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: 3,
            base_delay_ms: 2000,
        }
    }
}

impl From<Option<RetrySettings>> for RetryConfig {
    fn from(s: Option<RetrySettings>) -> Self {
        match s {
            Some(s) => Self {
                enabled: s.enabled.unwrap_or(true),
                max_retries: s.max_retries.unwrap_or(3),
                base_delay_ms: s.base_delay_ms.unwrap_or(2000),
            },
            None => Self::default(),
        }
    }
}

/// Check if an error message indicates a retryable error
pub fn is_retryable_error_message(err: &str) -> bool {
    let lower = err.to_lowercase();
    let patterns = [
        "overloaded",
        "rate.?limit",
        "too many requests",
        "429",
        "500",
        "502",
        "503",
        "504",
        "service.?unavailable",
        "server.?error",
        "internal.?error",
        "network.?error",
        "connection.?error",
        "connection.?refused",
        "connection.?lost",
        "fetch failed",
        "upstream.?connect",
        "reset before headers",
        "socket hang up",
        "timed? out",
        "timeout",
        "request failed",
        "stream error",
        "retry delay",
    ];
    patterns.iter().any(|p| {
        regex::Regex::new(p).map_or(false, |re| re.is_match(&lower))
    })
}

/// Run the agent loop with automatic retry on transient errors.
///
/// This wraps `run_agent_loop` / `run_agent_loop_continue` with exponential
/// backoff retry logic.
/// Retry events (AutoRetryStart/AutoRetryEnd) are emitted through the
/// on_event callback for UI feedback.
pub async fn run_agent_loop_with_retry(
    config: AgentLoopConfig,
    initial_messages: Vec<Message>,
    retry_config: RetryConfig,
    cancel_signal: Option<Arc<watch::Receiver<bool>>>,
) -> Result<AgentRunResult, String> {
    if !retry_config.enabled || retry_config.max_retries == 0 {
        return run_agent_loop(config, initial_messages).await;
    }

    for attempt in 1..=retry_config.max_retries.saturating_add(1) {
        let result = run_agent_loop(config.clone(), initial_messages.clone()).await;

        match result {
            Ok(success) => {
                let prev_attempt = attempt.saturating_sub(1);
                if prev_attempt > 0 {
                    if let Some(ref handler) = config.on_event {
                        handler(AgentEvent::AutoRetryEnd {
                            success: true,
                            attempt: prev_attempt,
                            final_error: None,
                        });
                    }
                }
                return Ok(success);
            }
            Err(err) => {
                if attempt >= retry_config.max_retries.saturating_add(1)
                    || !is_retryable_error_message(&err)
                {
                    if attempt > 1 {
                        if let Some(ref handler) = config.on_event {
                            handler(AgentEvent::AutoRetryEnd {
                                success: false,
                                attempt: attempt - 1,
                                final_error: Some(err.clone()),
                            });
                        }
                    }
                    return Err(err);
                }

                let delay_ms = retry_config.base_delay_ms * 2u64.pow(attempt.saturating_sub(1));

                if let Some(ref handler) = config.on_event {
                    handler(AgentEvent::AutoRetryStart {
                        attempt,
                        max_attempts: retry_config.max_retries,
                        delay_ms,
                        error_message: err.clone(),
                    });
                }

                // Sleep with cancellation support
                let delay = Duration::from_millis(delay_ms);
                if let Some(ref signal) = cancel_signal {
                    let mut rx = signal.as_ref().clone();
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {}
                        _ = rx.changed() => {
                            if let Some(ref handler) = config.on_event {
                                handler(AgentEvent::AutoRetryEnd {
                                    success: false,
                                    attempt,
                                    final_error: Some("Retry cancelled".to_string()),
                                });
                            }
                            return Err("Retry cancelled".to_string());
                        }
                    }
                } else {
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err("Max retries exceeded".to_string())
}

/// Run `run_agent_loop_continue` with automatic retry on transient errors.
/// Same retry logic as `run_agent_loop_with_retry` but uses `run_agent_loop_continue`.
async fn run_agent_loop_continue_with_retry(
    config: AgentLoopConfig,
    existing_messages: Vec<Message>,
    retry_config: RetryConfig,
    cancel_signal: Option<Arc<watch::Receiver<bool>>>,
) -> Result<AgentRunResult, String> {
    if !retry_config.enabled || retry_config.max_retries == 0 {
        return run_agent_loop_continue(config, existing_messages).await;
    }

    for attempt in 1..=retry_config.max_retries.saturating_add(1) {
        let result = run_agent_loop_continue(config.clone(), existing_messages.clone()).await;

        match result {
            Ok(success) => {
                let prev_attempt = attempt.saturating_sub(1);
                if prev_attempt > 0 {
                    if let Some(ref handler) = config.on_event {
                        handler(AgentEvent::AutoRetryEnd {
                            success: true,
                            attempt: prev_attempt,
                            final_error: None,
                        });
                    }
                }
                return Ok(success);
            }
            Err(err) => {
                if attempt >= retry_config.max_retries.saturating_add(1)
                    || !is_retryable_error_message(&err)
                {
                    if attempt > 1 {
                        if let Some(ref handler) = config.on_event {
                            handler(AgentEvent::AutoRetryEnd {
                                success: false,
                                attempt: attempt - 1,
                                final_error: Some(err.clone()),
                            });
                        }
                    }
                    return Err(err);
                }

                let delay_ms = retry_config.base_delay_ms * 2u64.pow(attempt.saturating_sub(1));

                if let Some(ref handler) = config.on_event {
                    handler(AgentEvent::AutoRetryStart {
                        attempt,
                        max_attempts: retry_config.max_retries,
                        delay_ms,
                        error_message: err.clone(),
                    });
                }

                let delay = Duration::from_millis(delay_ms);
                if let Some(ref signal) = cancel_signal {
                    let mut rx = signal.as_ref().clone();
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {}
                        _ = rx.changed() => {
                            if let Some(ref handler) = config.on_event {
                                handler(AgentEvent::AutoRetryEnd {
                                    success: false,
                                    attempt,
                                    final_error: Some("Retry cancelled".to_string()),
                                });
                            }
                            return Err("Retry cancelled".to_string());
                        }
                    }
                } else {
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err("Max retries exceeded".to_string())
}

/// Run the agent loop with retry AND automatic goal-driven continuation.
///
/// After each segment (a complete agent loop with retry), checks the
/// `get_follow_up_messages` hook. If it returns non-empty messages,
/// starts a new continuation segment using `run_agent_loop_continue`.
/// Continues until the hook returns empty or max continuation count is reached.
pub async fn run_agent_loop_with_retry_and_continuation(
    config: AgentLoopConfig,
    initial_messages: Vec<Message>,
    retry_config: RetryConfig,
    cancel_signal: Option<Arc<watch::Receiver<bool>>>,
) -> Result<AgentRunResult, String> {
    // First segment
    let result = run_agent_loop_with_retry(
        config.clone(), initial_messages, retry_config.clone(), cancel_signal.clone(),
    ).await?;

    let mut all_messages = result.messages;
    let mut total_usage = result.usage;

    // Continuation segments
    loop {
        let follow_up = config.get_follow_up_messages.as_ref()
            .map(|f| f(&AgentRunResult {
                messages: all_messages.clone(),
                usage: total_usage.clone(),
            }))
            .unwrap_or_default();

        if follow_up.is_empty() {
            break;
        }

        all_messages.extend(follow_up);

        let result = run_agent_loop_continue_with_retry(
            config.clone(), all_messages, retry_config.clone(), cancel_signal.clone(),
        ).await?;

        all_messages = result.messages;
        total_usage.input += result.usage.input;
        total_usage.output += result.usage.output;
        total_usage.cache_read += result.usage.cache_read;
        total_usage.cache_write += result.usage.cache_write;
        total_usage.total_tokens += result.usage.total_tokens;
        total_usage.cost.input += result.usage.cost.input;
        total_usage.cost.output += result.usage.cost.output;
        total_usage.cost.cache_read += result.usage.cost.cache_read;
        total_usage.cost.cache_write += result.usage.cost.cache_write;
        total_usage.cost.total += result.usage.cost.total;
    }

    Ok(AgentRunResult {
        messages: all_messages,
        usage: total_usage,
    })
}
