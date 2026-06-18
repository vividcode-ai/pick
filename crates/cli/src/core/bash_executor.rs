//! Bash command execution with streaming support and cancellation

use std::io::Write;

/// Result from bash execution
#[derive(Debug, Clone)]
pub struct BashResult {
    /// Combined stdout + stderr output (sanitized, possibly truncated)
    pub output: String,
    /// Process exit code (None if killed/cancelled)
    pub exit_code: Option<i32>,
    /// Whether the command was cancelled
    pub cancelled: bool,
    /// Whether the output was truncated
    pub truncated: bool,
    /// Path to temp file containing full output (if truncated)
    pub full_output_path: Option<String>,
}

/// Options for bash execution
pub struct BashExecutorOptions<'a> {
    /// Callback for streaming output chunks
    pub on_chunk: Option<Box<dyn Fn(&str) + Send + 'a>>,
    /// Signal for cancellation - set to true to cancel running command
    pub cancelled: bool,
}

impl Default for BashExecutorOptions<'_> {
    fn default() -> Self {
        Self {
            on_chunk: None,
            cancelled: false,
        }
    }
}

const DEFAULT_MAX_BYTES: usize = 512_000;
const MAX_OUTPUT_BYTES: usize = DEFAULT_MAX_BYTES * 2;

/// Execute a bash command locally with streaming output
pub async fn execute_bash(
    command: &str,
    cwd: &str,
    options: BashExecutorOptions<'_>,
) -> Result<BashResult, String> {
    use crate::core::exec::{ExecOptions, exec_command};

    let mut output_chunks: Vec<String> = Vec::new();
    let mut output_bytes: usize = 0;
    let mut total_bytes: usize = 0;
    let mut temp_file_path: Option<String> = None;
    let mut temp_file: Option<std::fs::File> = None;

    let exec_opts = ExecOptions {
        timeout_ms: None,
        cwd: Some(cwd.to_string()),
    };

    let shell_config = pick_agent::utils::get_shell_config(None)
        .map_err(|e| format!("Shell config error: {}", e))?;
    let mut shell_args = shell_config.args.clone();
    shell_args.push(command.to_string());
    let result = exec_command(&shell_config.shell, &shell_args, Some(&exec_opts)).await;

    let stdout = result.stdout;
    let stderr = result.stderr;
    let combined = if stderr.is_empty() {
        stdout.clone()
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    if !combined.is_empty() {
        total_bytes += combined.len();
        if total_bytes > DEFAULT_MAX_BYTES && temp_file.is_none() {
            let id = uuid::Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)).to_string()[..16]
                .to_string();
            let tmp_path = std::env::temp_dir().join(format!("Pick-bash-{}.log", id));
            if let Ok(file) = std::fs::File::create(&tmp_path) {
                temp_file_path = Some(tmp_path.to_string_lossy().to_string());
                temp_file = Some(file);
                for chunk_str in &output_chunks {
                    if let Some(ref mut f) = temp_file {
                        let _ = f.write_all(chunk_str.as_bytes());
                    }
                }
            }
        }

        if let Some(ref mut f) = temp_file {
            let _ = f.write_all(combined.as_bytes());
        }

        output_chunks.push(combined.clone());
        output_bytes += combined.len();
        while output_bytes > MAX_OUTPUT_BYTES && output_chunks.len() > 1 {
            let removed = output_chunks.remove(0);
            output_bytes -= removed.len();
        }

        if let Some(ref cb) = options.on_chunk {
            cb(&combined);
        }
    }

    let output_text = output_chunks.join("");

    let truncation_result = truncate_tail(&output_text);
    if truncation_result.truncated && temp_file.is_none() {
        let id =
            uuid::Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)).to_string()[..16].to_string();
        let tmp_path = std::env::temp_dir().join(format!("Pick-bash-{}.log", id));
        if let Ok(mut file) = std::fs::File::create(&tmp_path) {
            let _ = file.write_all(output_text.as_bytes());
            temp_file_path = Some(tmp_path.to_string_lossy().to_string());
        }
    }

    if let Some(mut f) = temp_file {
        let _ = f.flush();
    }

    Ok(BashResult {
        output: if truncation_result.truncated {
            truncation_result.content
        } else {
            output_text
        },
        exit_code: Some(result.exit_code),
        cancelled: options.cancelled,
        truncated: truncation_result.truncated,
        full_output_path: temp_file_path,
    })
}

struct TruncationResult {
    content: String,
    truncated: bool,
}

fn truncate_tail(text: &str) -> TruncationResult {
    if text.len() <= MAX_OUTPUT_BYTES {
        return TruncationResult {
            content: text.to_string(),
            truncated: false,
        };
    }

    let cutoff = MAX_OUTPUT_BYTES;
    let keep_prefix_len = cutoff.saturating_sub(100);
    let truncated = format!(
        "{}...\n[Output truncated at {} bytes]",
        &text[..keep_prefix_len],
        MAX_OUTPUT_BYTES
    );
    TruncationResult {
        content: truncated,
        truncated: true,
    }
}
