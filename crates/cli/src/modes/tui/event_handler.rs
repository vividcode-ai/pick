use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use pick_agent::core::events::AgentEvent;
use pick_ai::types::{ContentBlock, Message};
use tokio::sync::mpsc;

use super::types::TuiCommand;

/// Build the on_event callback for the agent loop
pub(crate) fn create_on_event(
    cmd_tx: mpsc::UnboundedSender<TuiCommand>,
    tool_start_times: Arc<Mutex<HashMap<String, Instant>>>,
    tool_args_map: Arc<Mutex<HashMap<String, serde_json::Value>>>,
) -> Arc<dyn Fn(AgentEvent) + Send + Sync> {
    let cmd_tx_for_events = cmd_tx;
    let tool_times = tool_start_times;
    let tool_args = tool_args_map;
    Arc::new(move |event| match event {
        AgentEvent::MessageUpdate { message, .. } => {
            if let Message::Assistant(msg) = message {
                let combined = format_message_content(&msg.content);
                if !combined.is_empty() {
                    let _ = cmd_tx_for_events.send(TuiCommand::StreamContent(combined));
                }
            }
        }
        AgentEvent::ToolExecutionUpdate {
            ref tool_call_id,
            ref tool_name,
            ref partial_result,
            ..
        } => {
            if tool_name == "todo_plan" {
                return;
            }
            let partial_output = partial_result
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !partial_output.is_empty() {
                let _ = cmd_tx_for_events.send(TuiCommand::ToolExecutionUpdate {
                    tool_call_id: tool_call_id.clone(),
                    partial_output: partial_output.to_string(),
                });
            }
        }
        AgentEvent::ToolExecutionStart {
            ref tool_name,
            ref tool_call_id,
            ref args,
            ..
        } => {
            if tool_name.to_lowercase() == "bash"
                && let Ok(mut times) = tool_times.lock() {
                    times.insert(tool_call_id.clone(), Instant::now());
                }
            if let Ok(mut map) = tool_args.lock() {
                map.insert(tool_call_id.clone(), args.clone());
            }
            let _ = cmd_tx_for_events.send(TuiCommand::ToolExecutionStart {
                tool_call_id: tool_call_id.clone(),
                tool_name: tool_name.clone(),
                args: args.clone(),
            });
        }
        AgentEvent::ToolExecutionEnd {
            ref tool_call_id,
            ref tool_name,
            result,
            is_error,
            ..
        } => {
            let raw_output = if is_error {
                result
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string()
            } else if let Some(content) = result.get("content") {
                if let Some(texts) = content.as_array() {
                    texts
                        .iter()
                        .filter_map(|t| t.as_str())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join("")
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            let output = format_tool_output(
                tool_call_id,
                tool_name,
                raw_output,
                is_error,
                &tool_args,
                &tool_times,
            );

            let _ = cmd_tx_for_events.send(TuiCommand::ToolExecutionEnd {
                tool_call_id: tool_call_id.clone(),
                tool_name: tool_name.clone(),
                output,
                is_error,
            });
        }
        AgentEvent::TurnEnd { .. } => {
            let _ = cmd_tx_for_events.send(TuiCommand::EndTurn);
        }
        AgentEvent::AutoRetryStart {
            attempt,
            max_attempts,
            delay_ms,
            ..
        } => {
            let msg = format!(
                "Auto-retrying ({}/{}) in {}s...",
                attempt,
                max_attempts,
                delay_ms / 1000
            );
            let _ = cmd_tx_for_events.send(TuiCommand::SetStatus(msg));
        }
        AgentEvent::AutoRetryEnd { success: false, .. } => {
            let _ = cmd_tx_for_events.send(TuiCommand::ClearStatus);
        }
        AgentEvent::TodoUpdated { ref todos } => {
            if let Some(arr) = todos.as_array() {
                let _ = cmd_tx_for_events.send(TuiCommand::UpdateTodos(arr.clone()));
            }
        }
        _ => {}
    })
}

/// Format message content blocks into a combined display string
fn format_message_content(content: &[ContentBlock]) -> String {
    let mut combined = String::new();
    for block in content {
        match block {
            ContentBlock::Text(t) => {
                combined.push_str(&t.text);
            }
            ContentBlock::Thinking(t)
                if !t.thinking.is_empty() => {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(&format!(
                        "\x1b[3m\x1b[38;2;128;128;128m{}\x1b[23m\x1b[39m\n\n",
                        t.thinking.trim_end()
                    ));
                }
            ContentBlock::Image(img) => {
                let rendered = render_image_block(img);
                if !rendered.is_empty() {
                    if !combined.is_empty() && !combined.ends_with('\n') {
                        combined.push('\n');
                    }
                    combined.push_str(&rendered);
                    combined.push('\n');
                }
            }
            _ => {}
        }
    }
    combined
}

/// Render image content block to ANSI terminal output
fn render_image_block(img: &pick_ai::types::ImageContent) -> String {
    let (mime, data) = if img.mime_type != "image/png" {
        use base64::Engine;
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&img.data) {
            if let Ok(png_bytes) =
                crate::utils::image::convert_image(&bytes, crate::utils::image::ImageFormat::Png)
            {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
                ("image/png".to_string(), b64)
            } else {
                (img.mime_type.clone(), img.data.clone())
            }
        } else {
            (img.mime_type.clone(), img.data.clone())
        }
    } else {
        (img.mime_type.clone(), img.data.clone())
    };

    let max_width = std::cmp::min(60u32, 60);
    let options = pick_tui::components::image_component::ImageOptions {
        max_width_cells: Some(max_width),
        ..Default::default()
    };
    let mut image = pick_tui::components::image_component::Image::new(data, mime, options, None);
    let image_lines = image.render(max_width as usize);

    if image_lines.is_empty() {
        return String::new();
    }

    let is_fallback = image_lines.len() == 1
        && (image_lines[0].starts_with("[Image:")
            || image_lines[0].contains("image not supported"));
    if is_fallback {
        image_lines[0].clone()
    } else {
        image_lines.join("\n")
    }
}

/// Format tool output based on tool type (edit diff, read highlight, bash timer, write highlight)
fn format_tool_output(
    tool_call_id: &str,
    tool_name: &str,
    raw_output: String,
    is_error: bool,
    tool_args: &Arc<Mutex<HashMap<String, serde_json::Value>>>,
    tool_times: &Arc<Mutex<HashMap<String, Instant>>>,
) -> String {
    let tl = tool_name.to_lowercase();

    if tl == "write" && !is_error {
        format_tool_write(tool_call_id, raw_output, tool_args)
    } else if tl == "read" && !raw_output.is_empty() && !is_error {
        format_tool_read(tool_call_id, raw_output, tool_args)
    } else if tl == "edit" && !raw_output.is_empty() {
        format_tool_edit(raw_output)
    } else if tl == "bash" {
        format_tool_bash(tool_call_id, raw_output, tool_times)
    } else {
        raw_output
    }
}

/// Format Write tool output with syntax highlighting
fn format_tool_write(
    tool_call_id: &str,
    output: String,
    tool_args: &Arc<Mutex<HashMap<String, serde_json::Value>>>,
) -> String {
    let stored_args = tool_args
        .lock()
        .ok()
        .and_then(|mut map| map.remove(tool_call_id));
    let file_path = stored_args
        .as_ref()
        .and_then(|args| args.get("file_path").or_else(|| args.get("path")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let file_content = stored_args
        .as_ref()
        .and_then(|args| args.get("content"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if let (Some(path), Some(content)) = (&file_path, &file_content) {
        let lang = crate::utils::syntax_highlight::detect_language_from_path(path);
        let body = if let Some(lang) = lang {
            let h = crate::utils::syntax_highlight::highlight_to_ansi(content, Some(lang));
            if h.is_empty() { content.clone() } else { h }
        } else {
            content.clone()
        };
        format!(
            "Successfully wrote {} bytes to {}\n{}",
            content.len(),
            path,
            body
        )
    } else if let Some(path) = file_path {
        let lang = crate::utils::syntax_highlight::detect_language_from_path(&path);
        if let Some(lang) = lang {
            let highlighted =
                crate::utils::syntax_highlight::highlight_to_ansi(&output, Some(lang));
            if !highlighted.is_empty() {
                return highlighted;
            }
        }
        output
    } else {
        output
    }
}

/// Format Read tool output with syntax highlighting
fn format_tool_read(
    tool_call_id: &str,
    output: String,
    tool_args: &Arc<Mutex<HashMap<String, serde_json::Value>>>,
) -> String {
    let stored_args = tool_args
        .lock()
        .ok()
        .and_then(|mut map| map.remove(tool_call_id));
    let file_path = stored_args
        .as_ref()
        .and_then(|args| args.get("file_path").or_else(|| args.get("path")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if let Some(path) = file_path {
        let lang = crate::utils::syntax_highlight::detect_language_from_path(&path);
        if let Some(lang) = lang {
            let highlighted =
                crate::utils::syntax_highlight::highlight_to_ansi(&output, Some(lang));
            if !highlighted.is_empty() {
                return highlighted;
            }
        }
    }
    output
}

/// Format Edit tool output with word-level diff coloring
fn format_tool_edit(output: String) -> String {
    const DIFF_RED: &str = "\x1b[38;2;204;102;102m";
    const DIFF_GREEN: &str = "\x1b[38;2;181;189;104m";
    const DIFF_GRAY: &str = "\x1b[38;2;128;128;128m";
    const INVERSE: &str = "\x1b[7m";
    const INVERSE_OFF: &str = "\x1b[27m";

    let lines: Vec<&str> = output.lines().collect();
    let mut colored = String::new();
    let mut i = 0;
    while i < lines.len() {
        if i + 1 < lines.len() {
            let deleted = lines[i].strip_prefix('-');
            let added = lines[i + 1].strip_prefix('+');
            if let (Some(old_text), Some(new_text)) = (deleted, added) {
                use similar::{ChangeTag, TextDiff};
                let word_diff = TextDiff::from_words(old_text, new_text);
                let mut old_part = String::new();
                let mut new_part = String::new();
                for change in word_diff.iter_all_changes() {
                    let value = change.value();
                    match change.tag() {
                        ChangeTag::Equal => {
                            old_part.push_str(value);
                            new_part.push_str(value);
                        }
                        ChangeTag::Delete => {
                            old_part.push_str(&format!("{}{}{}", INVERSE, value, INVERSE_OFF));
                        }
                        ChangeTag::Insert => {
                            new_part.push_str(&format!("{}{}{}", INVERSE, value, INVERSE_OFF));
                        }
                    }
                }
                colored.push_str(&format!("{}-{}\x1b[39m\n", DIFF_RED, old_part));
                colored.push_str(&format!("{}+{}\x1b[39m\n", DIFF_GREEN, new_part));
                i += 2;
                continue;
            }
        }
        if let Some(rest) = lines[i].strip_prefix('+') {
            colored.push_str(&format!("{}+{}\x1b[39m\n", DIFF_GREEN, rest));
        } else if let Some(rest) = lines[i].strip_prefix('-') {
            colored.push_str(&format!("{}-{}\x1b[39m\n", DIFF_RED, rest));
        } else {
            colored.push_str(&format!("{}{}\x1b[39m\n", DIFF_GRAY, lines[i]));
        }
        i += 1;
    }
    colored.trim_end().to_string()
}

/// Format Bash tool output with elapsed timer
fn format_tool_bash(
    tool_call_id: &str,
    output: String,
    tool_times: &Arc<Mutex<HashMap<String, Instant>>>,
) -> String {
    let elapsed = tool_times
        .lock()
        .ok()
        .and_then(|mut times| times.remove(tool_call_id));

    match elapsed {
        Some(start) => {
            let dur = start.elapsed();
            let duration_str = if dur.as_secs() >= 60 {
                format!("{}m {:02}s", dur.as_secs() / 60, dur.as_secs() % 60)
            } else {
                format!("{}.{:01}s", dur.as_secs(), dur.subsec_millis() / 100)
            };
            if !output.is_empty() {
                format!(
                    "{}\n\x1b[38;2;128;128;128mTook {}\x1b[39m",
                    output, duration_str
                )
            } else {
                format!("\x1b[38;2;128;128;128mTook {}\x1b[39m", duration_str)
            }
        }
        None => output,
    }
}
