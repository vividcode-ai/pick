use super::output_accumulator::OutputAccumulator;
use super::render_utils::{
    ToolRenderContext, ToolRenderOptions, ToolRenderOutput, ToolTheme, invalid_arg_text,
};

/// Tool definition for the bash tool
pub fn create_bash_tool_definition() -> BashToolDefinition {
    BashToolDefinition
}

pub struct BashToolDefinition;

impl BashToolDefinition {
    pub fn name(&self) -> &str {
        "bash"
    }

    pub fn description(&self) -> &str {
        "Execute a bash command in the current working directory. Returns stdout and stderr."
    }

    pub fn execute<'a>(
        &self,
        command: &str,
        cwd: &str,
        timeout: Option<u64>,
        signal: Option<&'a tokio::sync::watch::Receiver<bool>>,
    ) -> BashExecution<'a> {
        BashExecution {
            command: command.to_string(),
            cwd: cwd.to_string(),
            timeout,
            signal,
        }
    }
}

pub struct BashExecution<'a> {
    command: String,
    cwd: String,
    timeout: Option<u64>,
    signal: Option<&'a tokio::sync::watch::Receiver<bool>>,
}

impl BashExecution<'_> {
    pub async fn run(self) -> Result<BashOutput, String> {
        use tokio::process::Command;

        let mut output = OutputAccumulator::new(None, None, Some("Pick-bash"));
        let shell_config = pick_agent::utils::get_shell_config(None)
            .map_err(|e| format!("Shell config error: {}", e))?;
        let mut child = Command::new(&shell_config.shell)
            .args(&shell_config.args)
            .arg(&self.command)
            .current_dir(&self.cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Failed to spawn shell: {}", e))?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let mut stdout_reader = tokio::io::BufReader::new(stdout);
        let mut stderr_reader = tokio::io::BufReader::new(stderr);
        let mut buf = Vec::new();

        use tokio::io::AsyncReadExt;
        loop {
            buf.clear();
            let n = stdout_reader
                .read_buf(&mut buf)
                .await
                .map_err(|e| format!("Read error: {}", e))?;
            if n == 0 {
                break;
            }
            output.append(&buf);
        }
        loop {
            buf.clear();
            let n = stderr_reader
                .read_buf(&mut buf)
                .await
                .map_err(|e| format!("Read error: {}", e))?;
            if n == 0 {
                break;
            }
            output.append(&buf);
        }

        let status = child
            .wait()
            .await
            .map_err(|e| format!("Wait error: {}", e))?;
        output.finish();
        let snapshot = output.snapshot(false);
        output.close_temp_file();

        Ok(BashOutput {
            content: snapshot.content,
            exit_code: status.code(),
            truncated: snapshot.truncation.truncated,
            full_output_path: snapshot.full_output_path,
        })
    }
}

pub struct BashOutput {
    pub content: String,
    pub exit_code: Option<i32>,
    pub truncated: bool,
    pub full_output_path: Option<String>,
}

// ============================================================================
// Render Functions
// ============================================================================

fn format_duration_ms(ms: u128) -> String {
    format!("{:.1}s", ms as f64 / 1000.0)
}

/// Render a bash tool call — `$ command (timeout Xs)`
pub fn render_bash_call(args: &serde_json::Value, _ctx: &ToolRenderContext) -> ToolRenderOutput {
    let command = args.get("command").and_then(|v| v.as_str());
    let timeout = args.get("timeout").and_then(|v| v.as_u64());

    let timeout_suffix = match timeout {
        Some(t) => ToolTheme::fg("muted", &format!(" (timeout {}s)", t)),
        None => String::new(),
    };

    let command_display = match command {
        Some(c) => c.to_string(),
        None => invalid_arg_text(&|s| ToolTheme::fg("error", s)),
    };

    let label = format!(
        "{}{}",
        ToolTheme::fg(
            "toolTitle",
            &ToolTheme::bold(&format!("$ {}", command_display))
        ),
        timeout_suffix,
    );

    ToolRenderOutput {
        label,
        formatted: String::new(),
    }
}

/// Render a bash tool result — output with exit code and duration
pub fn render_bash_result(
    output: &BashOutput,
    options: &ToolRenderOptions,
    _ctx: &ToolRenderContext,
) -> ToolRenderOutput {
    let mut formatted = String::new();

    if !output.content.is_empty() {
        let styled: String = output
            .content
            .split('\n')
            .map(|line| ToolTheme::fg("toolOutput", line))
            .collect::<Vec<_>>()
            .join("\n");

        if options.expanded {
            formatted = format!("\n{}", styled);
        } else {
            let preview_lines: Vec<&str> = styled.split('\n').take(5).collect();
            let total = styled.split('\n').count();
            let skipped = total.saturating_sub(5);
            if skipped > 0 {
                formatted = format!(
                    "\n{}",
                    ToolTheme::fg(
                        "muted",
                        &format!("... ({} earlier lines, use expand to expand)", skipped)
                    )
                );
                if !preview_lines.is_empty() {
                    formatted.push('\n');
                    formatted.push_str(&preview_lines.join("\n"));
                }
            } else {
                formatted = format!("\n{}", preview_lines.join("\n"));
            }
        }
    }

    if output.truncated {
        let mut warnings = Vec::new();
        if let Some(ref path) = output.full_output_path {
            warnings.push(format!("Full output: {}", path));
        }
        if !warnings.is_empty() {
            formatted.push_str(&format!(
                "\n{}",
                ToolTheme::fg("warning", &format!("[{}]", warnings.join(". ")))
            ));
        }
    }

    if let Some(code) = output.exit_code {
        formatted.push_str(&format!(
            "\n{}",
            ToolTheme::fg("muted", &format!("Exit code {}", code))
        ));
    }

    ToolRenderOutput {
        label: String::new(),
        formatted,
    }
}
