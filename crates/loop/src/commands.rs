//! Loop command parser — parses `/loop`, `/loop-status`, etc.

use std::time::Duration;

/// Parsed loop command.
#[derive(Debug, Clone)]
pub enum LoopCommand {
    Create {
        interval: Duration,
        action: String,
        kind: String,
        flags: LoopFlags,
    },
    Status,
    Pause {
        job_id: Option<String>,
    },
    Resume {
        job_id: Option<String>,
    },
    Remove {
        job_id: String,
    },
    Clear,
    Now {
        job_id: String,
    },
    Stop,
    Help,
}

/// Flags for `/loop` creation.
#[derive(Debug, Clone, Default)]
pub struct LoopFlags {
    pub safe: bool,
    pub quiet: bool,
    pub ask_never: bool,
    pub max_runs: Option<u32>,
    pub max_failures: Option<u32>,
    pub verify: Option<String>,
    pub preflight: Option<String>,
    pub postrun: Option<String>,
    pub branch: Option<String>,
    pub git_checkpoint: bool,
    pub no_overlap: bool,
}

/// Parse a loop command from the input string.
pub fn parse_loop_command(input: &str) -> Result<LoopCommand, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Usage: /loop <interval> <prompt>".to_string());
    }

    // Check for subcommands
    if let Some(rest) = input.strip_prefix("status")
        && (rest.is_empty() || rest.starts_with(' '))
    {
        return Ok(LoopCommand::Status);
    }
    if let Some(rest) = input.strip_prefix("pause") {
        let id = rest.trim().to_string();
        return Ok(LoopCommand::Pause {
            job_id: if id.is_empty() { None } else { Some(id) },
        });
    }
    if let Some(rest) = input.strip_prefix("resume") {
        let id = rest.trim().to_string();
        return Ok(LoopCommand::Resume {
            job_id: if id.is_empty() { None } else { Some(id) },
        });
    }
    if let Some(rest) = input.strip_prefix("remove") {
        let id = rest.trim().to_string();
        if id.is_empty() {
            return Err("Usage: /loop-remove <job_id>".to_string());
        }
        return Ok(LoopCommand::Remove { job_id: id });
    }
    if input == "clear" {
        return Ok(LoopCommand::Clear);
    }
    if let Some(rest) = input.strip_prefix("now") {
        let id = rest.trim().to_string();
        if id.is_empty() {
            return Err("Usage: /loop-now <job_id>".to_string());
        }
        return Ok(LoopCommand::Now { job_id: id });
    }
    if input == "stop" || input == "stop " {
        return Ok(LoopCommand::Stop);
    }
    if input == "help" || input == "?" {
        return Ok(LoopCommand::Help);
    }

    // Parse create command: [interval] [--flags] <action>
    let (first, rest) = split_first(input);
    let interval = if first == "--" || first.starts_with("--") {
        // No interval given, default to 0 (idle-driven)
        Duration::from_secs(0)
    } else {
        match parse_interval(first) {
            Ok(d) => d,
            Err(_) => {
                // First word is not an interval, treat it as the action
                // with default interval 0 (idle-driven)
                Duration::from_secs(0)
            }
        }
    };

    // If we consumed first as interval, extract the rest as action;
    // otherwise the whole input is the action.
    let rest_str = if interval.as_secs() == 0 && parse_interval(first).is_err() {
        input.to_string()
    } else {
        rest.to_string()
    };

    if rest_str.is_empty() {
        return Err("Missing action for loop command".to_string());
    }

    // Parse flags from the rest string (e.g., --safe, --verify "cargo test")
    let (action, flags) = parse_flags(&rest_str);

    Ok(LoopCommand::Create {
        interval,
        action,
        kind: "prompt".to_string(),
        flags,
    })
}

/// Parse a duration string like "5s", "10m", "1h", "30" (default seconds), "0".
pub fn parse_interval(s: &str) -> Result<Duration, String> {
    let s = s.trim().to_lowercase();
    if let Some(secs) = s.strip_suffix('s') {
        let n: f64 = secs
            .parse()
            .map_err(|_| format!("Invalid interval: {}", s))?;
        Ok(Duration::from_secs_f64(n))
    } else if let Some(mins) = s.strip_suffix('m') {
        let n: f64 = mins
            .parse()
            .map_err(|_| format!("Invalid interval: {}", s))?;
        Ok(Duration::from_secs_f64(n * 60.0))
    } else if let Some(hours) = s.strip_suffix('h') {
        let n: f64 = hours
            .parse()
            .map_err(|_| format!("Invalid interval: {}", s))?;
        Ok(Duration::from_secs_f64(n * 3600.0))
    } else {
        // Default to seconds
        let secs: f64 = s.parse().map_err(|_| format!("Invalid interval: {}", s))?;
        Ok(Duration::from_secs_f64(secs))
    }
}

fn split_first(s: &str) -> (&str, &str) {
    let s = s.trim();
    match s.find(char::is_whitespace) {
        Some(pos) => (&s[..pos], s[pos..].trim_start()),
        None => (s, ""),
    }
}

/// Extract flags from the remainder of a loop command string.
/// Returns (remaining_text, parsed_flags).
/// Flags are removed from the text, leaving only the action/prompt.
fn parse_flags(mut input: &str) -> (String, LoopFlags) {
    let mut flags = LoopFlags::default();
    let mut parts: Vec<&str> = Vec::new();

    loop {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            break;
        }

        if let Some(val) = take_flag_value(trimmed, "--verify") {
            flags.verify = Some(val.0.to_string());
            input = val.1;
            continue;
        }
        if let Some(val) = take_flag_value(trimmed, "--preflight") {
            flags.preflight = Some(val.0.to_string());
            input = val.1;
            continue;
        }
        if let Some(val) = take_flag_value(trimmed, "--postrun") {
            flags.postrun = Some(val.0.to_string());
            input = val.1;
            continue;
        }
        if let Some(val) = take_flag_value(trimmed, "--branch") {
            flags.branch = Some(val.0.to_string());
            input = val.1;
            continue;
        }
        if let Some(val) = take_flag_value(trimmed, "--max-runs") {
            if let Ok(n) = val.0.parse::<u32>() {
                flags.max_runs = Some(n);
            }
            input = val.1;
            continue;
        }
        if let Some(val) = take_flag_value(trimmed, "--max-failures") {
            if let Ok(n) = val.0.parse::<u32>() {
                flags.max_failures = Some(n);
            }
            input = val.1;
            continue;
        }

        // Boolean flags
        if take_flag(trimmed, "--safe") {
            flags.safe = true;
            input = remove_word(trimmed, "--safe");
            continue;
        }
        if take_flag(trimmed, "--quiet") {
            flags.quiet = true;
            input = remove_word(trimmed, "--quiet");
            continue;
        }
        if take_flag(trimmed, "--ask-never") {
            flags.ask_never = true;
            input = remove_word(trimmed, "--ask-never");
            continue;
        }
        if take_flag(trimmed, "--no-overlap") {
            flags.no_overlap = true;
            input = remove_word(trimmed, "--no-overlap");
            continue;
        }
        if take_flag(trimmed, "--git-checkpoint") {
            flags.git_checkpoint = true;
            input = remove_word(trimmed, "--git-checkpoint");
            continue;
        }

        // Not a flag → rest is the action text
        let (word, rest) = split_first(trimmed);
        parts.push(word);
        input = rest;
    }

    (parts.join(" "), flags)
}

/// Check if text starts with flag, returns (value, rest) for --flag "val".
fn take_flag_value<'a>(text: &'a str, flag: &str) -> Option<(&'a str, &'a str)> {
    let text = text.trim();
    if let Some(after) = text.strip_prefix(flag) {
        let after = after.trim();
        if after.is_empty() {
            return None;
        }
        // Quoted value: --flag "value"
        if let Some(val) = after.strip_prefix('"')
            && let Some(pos) = val.find('"')
        {
            return Some((&val[..pos], val[pos + 1..].trim_start()));
        }
        // Unquoted value: --flag value (space-separated)
        let (val, rest) = split_first(after);
        if !val.is_empty() {
            return Some((val, rest));
        }
    }
    None
}

/// Check if text starts with flag (bool, no value).
fn take_flag(text: &str, flag: &str) -> bool {
    let text = text.trim();
    text == flag
        || text.starts_with(&format!("{} ", flag))
        || text.starts_with(&format!("{}  ", flag))
}

/// Remove the first occurrence of a word from the text.
fn remove_word<'a>(text: &'a str, word: &str) -> &'a str {
    if let Some(pos) = text.find(word) {
        let before = &text[..pos].trim_end();
        let after = &text[pos + word.len()..].trim_start();
        if before.is_empty() { after } else { before }
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_interval() {
        assert_eq!(parse_interval("5s").unwrap(), Duration::from_secs(5));
        assert_eq!(parse_interval("10m").unwrap(), Duration::from_secs(600));
        assert_eq!(parse_interval("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_interval("30").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_interval("0").unwrap(), Duration::from_secs(0));
        assert!(parse_interval("abc").is_err());
    }

    #[test]
    fn test_parse_subcommands() {
        assert!(matches!(
            parse_loop_command("status").unwrap(),
            LoopCommand::Status
        ));
        assert!(matches!(
            parse_loop_command("pause").unwrap(),
            LoopCommand::Pause { .. }
        ));
        assert!(
            matches!(parse_loop_command("pause xyz").unwrap(), LoopCommand::Pause { job_id: Some(id) } if id == "xyz")
        );
        assert!(matches!(
            parse_loop_command("resume xyz").unwrap(),
            LoopCommand::Resume { .. }
        ));
        assert!(matches!(
            parse_loop_command("remove xyz").unwrap(),
            LoopCommand::Remove { .. }
        ));
        assert!(parse_loop_command("remove").is_err());
        assert!(matches!(
            parse_loop_command("clear").unwrap(),
            LoopCommand::Clear
        ));
        assert!(matches!(
            parse_loop_command("now xyz").unwrap(),
            LoopCommand::Now { .. }
        ));
        assert!(parse_loop_command("now").is_err());
        assert!(matches!(
            parse_loop_command("stop").unwrap(),
            LoopCommand::Stop
        ));
        assert!(matches!(
            parse_loop_command("help").unwrap(),
            LoopCommand::Help
        ));
    }

    #[test]
    fn test_parse_create_with_interval() {
        let cmd = parse_loop_command("30s fix the build").unwrap();
        match cmd {
            LoopCommand::Create {
                interval, action, ..
            } => {
                assert_eq!(interval, Duration::from_secs(30));
                assert_eq!(action, "fix the build");
            }
            _ => panic!("Expected Create"),
        }
    }

    #[test]
    fn test_parse_create_without_interval() {
        let cmd = parse_loop_command("fix the build").unwrap();
        match cmd {
            LoopCommand::Create {
                interval, action, ..
            } => {
                assert_eq!(interval, Duration::from_secs(0));
                assert_eq!(action, "fix the build");
            }
            _ => panic!("Expected Create"),
        }
    }

    #[test]
    fn test_parse_create_empty() {
        assert!(parse_loop_command("").is_err());
    }
}
