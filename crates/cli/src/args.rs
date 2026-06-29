//! CLI argument parsing

use std::collections::HashMap;

/// Value of an unknown flag (extension-registered flags)
#[derive(Debug, Clone)]
pub enum ArgValue {
    Bool(bool),
    String(String),
}

/// CLI diagnostic message
#[derive(Debug, Clone)]
pub struct CliDiagnostic {
    pub diag_type: String, // "warning" | "error"
    pub message: String,
}

/// Valid thinking levels
const VALID_THINKING_LEVELS: &[&str] = &["off", "minimal", "low", "medium", "high", "xhigh"];

/// Parsed CLI arguments
#[derive(Debug, Default)]
pub struct Args {
    pub mode: String,
    pub help: bool,
    pub version: bool,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub messages: Vec<String>,
    pub session: Option<String>,
    pub resume: bool,
    pub r#continue: Option<String>,
    pub fork: Option<String>,
    pub no_session: bool,
    pub no_tools: bool,
    pub no_builtin_tools: bool,
    pub tools: Vec<String>,
    pub api_key: Option<String>,
    pub thinking: Option<String>,
    pub verbose: bool,
    pub extensions: Vec<String>,
    pub skills: Vec<String>,
    pub offline: bool,
    pub system_prompt: Option<String>,
    pub append_system_prompt: Vec<String>,
    pub session_dir: Option<String>,
    pub models: Vec<String>,
    pub print: bool,
    pub update: bool,
    pub export_html: Option<String>,
    pub no_extensions: bool,
    pub no_skills: bool,
    pub no_themes: bool,
    pub no_context_files: bool,
    pub list_models: Option<String>,
    pub themes: Vec<String>,
    pub file_args: Vec<String>,
    /// Agent mode (build/plan)
    pub agent_mode: Option<String>,
    /// Audit command
    pub audit: bool,
    pub audit_recent: Option<usize>,
    pub audit_tool: Option<String>,
    pub audit_decision: Option<String>,
    pub audit_layer: Option<String>,
    pub audit_json: bool,
    /// Web serve mode
    pub port: Option<u16>,
    pub host: Option<String>,
    pub open_browser: bool,
    /// Unknown flags (potentially extension flags) - map of flag name to value
    pub unknown_flags: HashMap<String, ArgValue>,
    /// Diagnostics collected during parsing (warnings, errors)
    pub diagnostics: Vec<CliDiagnostic>,
}

/// Parse command-line arguments
pub fn parse_args(args: Vec<String>) -> Args {
    let mut parsed = Args::default();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--help" | "-h" => parsed.help = true,
            "--version" | "-V" => parsed.version = true,
            "--model" | "-m" => {
                i += 1;
                if i < args.len() {
                    parsed.model = Some(args[i].clone());
                }
            }
            "--provider" | "-p" => {
                i += 1;
                if i < args.len() {
                    parsed.provider = Some(args[i].clone());
                }
            }
            "--session" | "-s" => {
                i += 1;
                if i < args.len() {
                    parsed.session = Some(args[i].clone());
                }
            }
            "--resume" | "-r" => parsed.resume = true,
            "--continue" | "-c" => {
                parsed.r#continue = if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    i += 1;
                    Some(args[i].clone())
                } else {
                    Some(String::new())
                };
            }
            "--fork" => {
                i += 1;
                if i < args.len() {
                    parsed.fork = Some(args[i].clone());
                }
            }
            "--no-session" => parsed.no_session = true,
            "--no-tools" | "-nt" => parsed.no_tools = true,
            "--no-builtin-tools" | "-nbt" => parsed.no_builtin_tools = true,
            "--api-key" => {
                i += 1;
                if i < args.len() {
                    parsed.api_key = Some(args[i].clone());
                }
            }
            "--thinking" => {
                i += 1;
                if i < args.len() {
                    let level = &args[i];
                    if VALID_THINKING_LEVELS.contains(&level.as_str()) {
                        parsed.thinking = Some(level.clone());
                    } else {
                        parsed.diagnostics.push(CliDiagnostic {
                            diag_type: "warning".to_string(),
                            message: format!(
                                "Invalid thinking level \"{}\". Valid values: {}",
                                level,
                                VALID_THINKING_LEVELS.join(", ")
                            ),
                        });
                    }
                }
            }
            "--verbose" | "-v" => parsed.verbose = true,
            "--offline" => parsed.offline = true,
            "--mode" => {
                i += 1;
                if i < args.len() {
                    parsed.mode = args[i].clone();
                }
            }
            "--extension" | "-e" => {
                i += 1;
                if i < args.len() {
                    parsed.extensions.push(args[i].clone());
                }
            }
            "--tools" | "-t" => {
                i += 1;
                if i < args.len() {
                    for tool_name in args[i].split(',') {
                        let trimmed = tool_name.trim().to_string();
                        if !trimmed.is_empty() {
                            parsed.tools.push(trimmed);
                        }
                    }
                }
            }
            "--skill" => {
                i += 1;
                if i < args.len() {
                    parsed.skills.push(args[i].clone());
                }
            }
            "--system-prompt" => {
                i += 1;
                if i < args.len() {
                    parsed.system_prompt = Some(args[i].clone());
                }
            }
            "--append-system-prompt" => {
                i += 1;
                if i < args.len() {
                    parsed.append_system_prompt.push(args[i].clone());
                }
            }
            "--session-dir" => {
                i += 1;
                if i < args.len() {
                    parsed.session_dir = Some(args[i].clone());
                }
            }
            "--models" => {
                i += 1;
                if i < args.len() {
                    parsed.models = args[i].split(',').map(|s| s.trim().to_string()).collect();
                }
            }
            "--print" | "-P" => {
                parsed.print = true;
                if i + 1 < args.len() {
                    let next = args[i + 1].clone();
                    if !next.starts_with('-') || next.starts_with("---") {
                        parsed.messages.push(next);
                        i += 1;
                    }
                }
            }
            "--export" => {
                i += 1;
                if i < args.len() {
                    parsed.export_html = Some(args[i].clone());
                }
            }
            "--no-extensions" | "-ne" => parsed.no_extensions = true,
            "--no-skills" | "-ns" => parsed.no_skills = true,
            "--no-themes" => parsed.no_themes = true,
            "--no-context-files" | "-nc" => parsed.no_context_files = true,
            "--theme" => {
                i += 1;
                if i < args.len() {
                    parsed.themes.push(args[i].clone());
                }
            }
            "--agent-mode" => {
                i += 1;
                if i < args.len() {
                    parsed.agent_mode = Some(args[i].clone());
                }
            }
            "--list-models" => {
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    i += 1;
                    parsed.list_models = Some(args[i].clone());
                } else {
                    parsed.list_models = Some(String::new());
                }
            }
            "--audit" => parsed.audit = true,
            "--recent" => {
                i += 1;
                if i < args.len() {
                    parsed.audit_recent = args[i].parse().ok();
                }
            }
            "--tool" => {
                i += 1;
                if i < args.len() {
                    parsed.audit_tool = Some(args[i].clone());
                }
            }
            "--decision" => {
                i += 1;
                if i < args.len() {
                    parsed.audit_decision = Some(args[i].clone());
                }
            }
            "--layer" => {
                i += 1;
                if i < args.len() {
                    parsed.audit_layer = Some(args[i].clone());
                }
            }
            "--json" => parsed.audit_json = true,
            "--port" => {
                i += 1;
                if i < args.len() {
                    parsed.port = args[i].parse::<u16>().ok();
                }
            }
            "--host" => {
                i += 1;
                if i < args.len() {
                    parsed.host = Some(args[i].clone());
                }
            }
            "--open" => parsed.open_browser = true,
            _ => {
                if arg.starts_with("--") {
                    // Unknown flag (potentially extension-registered)
                    let eq_index = arg.find('=');
                    if let Some(idx) = eq_index {
                        let flag_name = arg[2..idx].to_string();
                        let flag_value = arg[idx + 1..].to_string();
                        parsed
                            .unknown_flags
                            .insert(flag_name, ArgValue::String(flag_value));
                    } else {
                        let flag_name = arg[2..].to_string();
                        if i + 1 < args.len() {
                            let next = &args[i + 1];
                            if !next.starts_with('-') && !next.starts_with('@') {
                                parsed
                                    .unknown_flags
                                    .insert(flag_name, ArgValue::String(next.clone()));
                                i += 1;
                            } else {
                                parsed.unknown_flags.insert(flag_name, ArgValue::Bool(true));
                            }
                        } else {
                            parsed.unknown_flags.insert(flag_name, ArgValue::Bool(true));
                        }
                    }
                } else if arg.starts_with('-') && !arg.starts_with("--") {
                    parsed.diagnostics.push(CliDiagnostic {
                        diag_type: "error".to_string(),
                        message: format!("Unknown option: {}", arg),
                    });
                } else if arg.starts_with('@') {
                    parsed.file_args.push(arg[1..].to_string());
                } else if !arg.starts_with('-') {
                    parsed.messages.push(arg.clone());
                }
            }
        }
        i += 1;
    }

    // Check if first positional argument is "update" subcommand
    if !parsed.messages.is_empty() && parsed.messages[0] == "update" && !parsed.print {
        parsed.update = true;
        parsed.messages.remove(0);
    }

    // Check if first positional argument is "serve" subcommand
    if !parsed.messages.is_empty()
        && parsed.messages[0] == "serve"
        && parsed.mode.is_empty()
        && !parsed.print
    {
        parsed.mode = "serve".to_string();
        parsed.messages.remove(0);
    }

    parsed
}

/// Print help information
pub fn print_help() {
    println!("Pick - AI coding agent");
    println!();
    println!("USAGE:");
    println!("  Pick [OPTIONS] [@files...] [MESSAGE...]");
    println!();
    println!("OPTIONS:");
    println!("  -h, --help              Print help");
    println!("  -V, --version           Print version");
    println!("  -m, --model <MODEL>     Specify the model to use");
    println!("  -p, --provider <PROV>   Specify the provider");
    println!("  -s, --session <ID>      Resume a session by ID");
    println!("  -r, --resume            Interactive session selector");
    println!("  -c, --continue [ID]     Continue a session (most recent, or by session ID)");
    println!("  --fork <ID>             Fork a session");
    println!("  --no-session            Run without session persistence");
    println!("  -nt, --no-tools         Disable all tools");
    println!("  -nbt, --no-builtin-tools Disable built-in tools only");
    println!("  --api-key <KEY>         Set API key");
    println!("  -t, --tools <TOOLS>     Comma-separated allowlist of tool names to enable");
    println!("  --thinking <LEVEL>      Set thinking level (off|minimal|low|medium|high|xhigh)");
    println!("  -v, --verbose           Verbose output");
    println!("  --offline               Offline mode");
    println!("  --mode <MODE>           Run mode (interactive|print|json|rpc|tui)");
    println!("  -e, --extension <PATH>  Load extension");
    println!("  --skill <PATH>          Load skill");
    println!("  --system-prompt <TEXT>  Set system prompt");
    println!("  --append-system-prompt  Append text to system prompt (can be used multiple times)");
    println!("  --session-dir <PATH>    Custom session directory");
    println!("  -P, --print             Print mode (batch)");
    println!("  --export <FILE>         Export session to HTML file");
    println!("  -ne, --no-extensions    Disable extensions");
    println!("  -ns, --no-skills        Disable skills");
    println!("  --no-themes             Disable themes");
    println!("  -nc, --no-context-files Disable context files");
    println!("  --list-models [FILTER]  List available models");
    println!();
    println!("  update                  Update Pick to the latest version");
    println!("  serve                   Start web server with SPA");
    println!("    --port <PORT>           Port (default: random available)");
    println!("    --host <HOST>           Host address (default: 127.0.0.1)");
    println!("    --open                  Open browser automatically");
    println!();
    println!("AUDIT:");
    println!("  --audit                 View permission audit trail");
    println!("  --recent <N>            Show last N events (default: 20)");
    println!("  --tool <NAME>           Filter by tool name");
    println!("  --decision <TYPE>       Filter by decision (allow|deny|ask)");
    println!("  --layer <LAYER>         Filter by audit layer");
    println!("  --json                  Output as JSON lines instead of table");
}
