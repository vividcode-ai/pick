//! Bash tool - executes shell commands with streaming output

use pick_ai::types::ContentBlock;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::core::state::{AgentTool, AgentToolResult, ToolContext, ToolExecutionMode};
use crate::utils::get_shell_config;

/// Read streamed output from a child process, forwarding chunks as progress updates
async fn read_stream<R: AsyncRead + Unpin>(
    reader: &mut tokio::io::BufReader<R>,
    progress: Option<&tokio::sync::mpsc::UnboundedSender<String>>,
) -> String {
    let mut output = String::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let n = reader.read_buf(&mut buf).await.unwrap_or(0);
        if n == 0 {
            break;
        }
        if let Ok(s) = String::from_utf8(buf.clone()) {
            output.push_str(&s);
            if let Some(tx) = progress {
                let _ = tx.send(s);
            }
        }
    }
    output
}

/// Platform info string injected into descriptions and error messages
fn platform_info() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let shell = match os {
        "windows" => "PowerShell",
        "macos" => "zsh",
        _ => "bash",
    };
    format!("Platform: {} / {}, shell: {}", os, arch, shell)
}

/// Create the bash tool definition with streaming progress support
pub fn create_bash_tool() -> AgentTool {
    let plat = platform_info();

    let params = pick_ai::types::JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(
            vec![
                (
                    "command".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": format!("Bash command to execute (required). {}", plat)
                    }),
                ),
                (
                    "timeout".to_string(),
                    serde_json::json!({
                        "type": "number",
                        "description": "Timeout in seconds (optional, no default timeout)"
                    }),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        required: Some(vec!["command".to_string()]),
        description: Some(format!(
            "Execute a command on the local system. Returns stdout and stderr. {}. Example: bash(command: \"ls -la\", timeout: 30)",
            plat
        )),
        items: None,
        additional_properties: Some(false),
    };

    AgentTool {
        name: "bash".to_string(),
        description: format!(
            "Execute a command on the local system. Returns stdout and stderr. {}. Example: bash(command: \"ls -la\", timeout: 30)",
            plat
        ),
        prompt_snippet: Some("Execute bash commands (ls, grep, find, etc.)".to_string()),
        prompt_guidelines: vec![],
        label: "bash".to_string(),
        parameters: params,
        execute: std::sync::Arc::new(move |_tool_call_id, args, ctx: ToolContext| {
            let plat = plat.clone();
            Box::pin(async move {
                let command = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("{} Missing command argument", plat))?;

                // Check ExecPolicy if available
                if let Some(ref pm) = ctx.permission_manager {
                    if let Some(reason) = pm.check_exec_policy(command) {
                        return Ok(AgentToolResult {
                            content: vec![ContentBlock::text(format!("Error: {}", reason))],
                            is_error: true,
                            terminate: false,
                        });
                    }
                    // If the exec policy returned Prompt (not a hard deny), ask the user
                    if let Some(ref ep) = pm.exec_policy
                        && matches!(
                            ep.evaluate(command),
                            crate::permission::exec_policy::ExecDecision::Prompt
                        )
                        && let Some(ref approve) = ctx.approve
                    {
                        let approved = approve(
                            "Exec Policy".to_string(),
                            format!("Command '{}' may be dangerous.", command),
                        )
                        .await;
                        if !approved {
                            return Ok(AgentToolResult {
                                content: vec![ContentBlock::text(
                                    "Error: Command blocked by user — exec policy required approval",
                                )],
                                is_error: true,
                                terminate: false,
                            });
                        }
                    }
                }

                let shell_config = get_shell_config(None)
                    .map_err(|e| format!("{} Failed to get shell config: {}", plat, e))?;

                // ---- Sandbox execution path ----
                // Resolve sandbox timeout from config, falling back to 120 seconds
                let sandbox_timeout = if let Some(ref pm) = ctx.permission_manager
                    && let Some(ref sc) = pm.sandbox_config
                {
                    sc.timeout_secs
                } else {
                    120
                };

                // Step 1: Try direct_spawn (Windows: CreateProcessAsUserW with restricted token)
                if let Some(sandbox) = ctx.sandbox.as_ref()
                    && let Some(cwd) = ctx.cwd.as_ref()
                {
                    let mut shell_args = shell_config.args.clone();
                    shell_args.push(command.to_string());
                    let mut req = crate::permission::sandbox::SandboxRequest::new(
                        &shell_config.shell,
                        &shell_args,
                        cwd,
                        ctx.fs_policy.clone(),
                        sandbox_timeout,
                    );
                    if let Some(ref pm) = ctx.permission_manager
                        && let Some(ref sc) = pm.sandbox_config
                    {
                        req.network_access = sc.network_access.clone();
                    }
                    if sandbox.is_available()
                        && let Some(result) = sandbox.direct_spawn(command, &req)
                    {
                        match result {
                            Ok((exit_code, stdout, stderr)) => {
                                let mut output = stdout;
                                if !stderr.is_empty() {
                                    if !output.is_empty() {
                                        output.push('\n');
                                    }
                                    output.push_str(&stderr);
                                }
                                if output.is_empty() {
                                    output =
                                        format!("Command completed with exit code: {}", exit_code);
                                }
                                return Ok(AgentToolResult {
                                    content: vec![ContentBlock::text(output)],
                                    is_error: exit_code != 0,
                                    terminate: false,
                                });
                            }
                            Err(e) => {
                                return Ok(AgentToolResult {
                                    content: vec![ContentBlock::text(format!("Error: {}", e))],
                                    is_error: true,
                                    terminate: false,
                                });
                            }
                        }
                    }
                }

                // Step 2: Try transform + spawn (Linux/macOS: bwrap/seatbelt wrapping)
                let sandbox_prog: Option<String>;
                let sandbox_args: Option<Vec<String>>;
                let use_sandbox = 'sandbox: {
                    if let Some(ref sandbox) = ctx.sandbox
                        && let Some(ref cwd) = ctx.cwd
                    {
                        let mut shell_args = shell_config.args.clone();
                        shell_args.push(command.to_string());
                        let req = crate::permission::sandbox::SandboxRequest::new(
                            &shell_config.shell,
                            &shell_args,
                            cwd,
                            ctx.fs_policy.clone(),
                            sandbox_timeout,
                        );
                        if sandbox.is_available() {
                            match sandbox.transform(&req) {
                                Ok((prog, args)) => {
                                    sandbox_prog = Some(prog);
                                    sandbox_args = Some(args);
                                    break 'sandbox true;
                                }
                                Err(e) => {
                                    return Err(e);
                                }
                            }
                        }
                    }
                    sandbox_prog = None;
                    sandbox_args = None;
                    false
                };

                // Pre-check: absolute path access control + external directory authorization
                if let (Some(ref fp), Some(ref cwd)) = (ctx.fs_policy, ctx.cwd)
                    && !fp.allow_absolute_paths()
                {
                    let abs_paths =
                        crate::permission::fs_policy::extract_absolute_path_args(command);
                    for path_str in &abs_paths {
                        let p = std::path::Path::new(path_str);
                        if !p.is_absolute() {
                            continue;
                        }
                        let is_denied = match fp.resolve_access(p, cwd) {
                            Ok(crate::permission::fs_policy::AccessMode::Deny) | Err(_) => true,
                            _ => false,
                        };
                        if !is_denied {
                            continue;
                        }

                        // Protected paths (e.g. .git/**) are hard denied, not authorizable
                        if fp.is_path_protected(p, cwd).unwrap_or(false) {
                            return Ok(AgentToolResult {
                                content: vec![ContentBlock::text(format!(
                                    "Error: Path access denied: '{}' is a protected path",
                                    path_str
                                ))],
                                is_error: true,
                                terminate: false,
                            });
                        }

                        // Path is outside workspace — check authorization
                        let authorized = if let Some(ref pm) = ctx.permission_manager {
                            crate::permission::external_dir::check_authorization(
                                "Bash",
                                path_str,
                                pm,
                                ctx.question.as_ref(),
                            )
                            .await?
                        } else {
                            false
                        };

                        if !authorized {
                            return Ok(AgentToolResult {
                                content: vec![ContentBlock::text(format!(
                                    "Error: Path access denied: '{}' is outside the allowed workspace",
                                    path_str
                                ))],
                                is_error: true,
                                terminate: false,
                            });
                        }
                    }
                }

                let mut cmd = if use_sandbox {
                    let mut c = tokio::process::Command::new(sandbox_prog.as_ref().unwrap());
                    c.args(sandbox_args.as_ref().unwrap());
                    c
                } else {
                    let mut c = tokio::process::Command::new(&shell_config.shell);
                    c.args(&shell_config.args).arg(command);
                    c
                };
                cmd.stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .kill_on_drop(true);

                let timeout_secs = args.get("timeout").and_then(|v| v.as_u64());

                // Spawn child process with piped stdio for streaming
                let mut child = cmd
                    .spawn()
                    .map_err(|e| format!("{} Failed to spawn command: {}", plat, e))?;

                let stdout = child.stdout.take().expect("stdout not configured");
                let stderr = child.stderr.take().expect("stderr not configured");
                let mut stdout_reader = tokio::io::BufReader::new(stdout);
                let mut stderr_reader = tokio::io::BufReader::new(stderr);

                let progress = ctx.progress.as_ref();

                let (stdout_result, stderr_result) = tokio::join!(
                    read_stream(&mut stdout_reader, progress),
                    read_stream(&mut stderr_reader, None),
                );

                let status = if let Some(secs) = timeout_secs {
                    let dur = std::time::Duration::from_secs(secs);
                    match tokio::time::timeout(dur, child.wait()).await {
                        Ok(Ok(s)) => s,
                        Ok(Err(e)) => {
                            return Err(format!("{} Failed to wait for command: {}", plat, e));
                        }
                        Err(_) => {
                            let _ = child.kill().await;
                            return Ok(AgentToolResult {
                                content: vec![ContentBlock::text(format!(
                                    "{} Command timed out after {} seconds",
                                    plat, secs
                                ))],
                                is_error: true,
                                terminate: false,
                            });
                        }
                    }
                } else {
                    child
                        .wait()
                        .await
                        .map_err(|e| format!("{} Failed to wait for command: {}", plat, e))?
                };

                let mut result_text = stdout_result;

                if !stderr_result.is_empty() {
                    if !result_text.is_empty() {
                        result_text.push_str("\n--- stderr ---\n");
                    }
                    result_text.push_str(&stderr_result);
                }

                if result_text.is_empty() {
                    result_text = format!(
                        "Command completed with exit code: {}",
                        status.code().unwrap_or(-1)
                    );
                }

                let is_error = !status.success();
                let output = if is_error {
                    format!("{}\n{}", plat, result_text)
                } else {
                    result_text
                };

                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(output)],
                    is_error,
                    terminate: false,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}
