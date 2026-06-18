//! Resolve configuration values that may be shell commands, environment variables, or literals


use std::collections::HashMap;
use std::sync::Mutex;

use tokio::process::Command;

static COMMAND_RESULT_CACHE: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);

/// Resolve a config value to an actual value.
/// - If starts with "!", executes the rest as a shell command and uses stdout (cached)
/// - Otherwise checks environment variable first, then treats as literal
pub fn resolve_config_value(config: &str) -> Option<String> {
    if config.starts_with('!') {
        return execute_command_cached(config);
    }
    std::env::var(config).ok().or_else(|| Some(config.to_string()))
}

/// Resolve without using the cache
pub fn resolve_config_value_uncached(config: &str) -> Option<String> {
    if config.starts_with('!') {
        return execute_command_uncached(config);
    }
    std::env::var(config).ok().or_else(|| Some(config.to_string()))
}

/// Resolve a config value or return an error message
pub fn resolve_config_value_or_throw(config: &str, description: &str) -> Result<String, String> {
    let resolved = resolve_config_value_uncached(config);
    match resolved {
        Some(v) => Ok(v),
        None => {
            if config.starts_with('!') {
                Err(format!(
                    "Failed to resolve {} from shell command: {}",
                    description,
                    &config[1..]
                ))
            } else {
                Err(format!("Failed to resolve {}", description))
            }
        }
    }
}

/// Resolve all header values
pub fn resolve_headers(headers: Option<&HashMap<String, String>>) -> Option<HashMap<String, String>> {
    let headers = headers?;
    let mut resolved = HashMap::new();
    for (key, value) in headers {
        if let Some(resolved_value) = resolve_config_value(value) {
            resolved.insert(key.clone(), resolved_value);
        }
    }
    if resolved.is_empty() { None } else { Some(resolved) }
}

/// Resolve all header values or throw on failure
pub fn resolve_headers_or_throw(
    headers: Option<&HashMap<String, String>>,
    description: &str,
) -> Result<Option<HashMap<String, String>>, String> {
    let headers = match headers {
        Some(h) => h,
        None => return Ok(None),
    };
    let mut resolved = HashMap::new();
    for (key, value) in headers {
        let resolved_value = resolve_config_value_or_throw(value, &format!("{} header \"{}\"", description, key))?;
        resolved.insert(key.clone(), resolved_value);
    }
    if resolved.is_empty() { Ok(None) } else { Ok(Some(resolved)) }
}

/// Clear the config value command cache
pub fn clear_config_value_cache() {
    if let Ok(mut cache) = COMMAND_RESULT_CACHE.lock() {
        *cache = None;
    }
}

fn execute_command_cached(config: &str) -> Option<String> {
    let mut cache = COMMAND_RESULT_CACHE.lock().unwrap();
    let cache = cache.get_or_insert_with(HashMap::new);

    if let Some(result) = cache.get(config) {
        return result.clone();
    }

    let result = execute_command_uncached(config);
    cache.insert(config.to_string(), result.clone());
    result
}

fn execute_command_uncached(config: &str) -> Option<String> {
    let command = &config[1..];

    #[cfg(target_os = "windows")]
    {
        let configured_result = execute_with_configured_shell(command);
        if configured_result.executed {
            return configured_result.value;
        }
        execute_with_default_shell(command)
    }

    #[cfg(not(target_os = "windows"))]
    execute_with_default_shell(command)
}

struct ShellResult {
    executed: bool,
    value: Option<String>,
}

#[cfg(target_os = "windows")]
fn execute_with_configured_shell(command: &str) -> ShellResult {
    let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
    let args = ["/c", "/d", "/s"];
    execute_shell_command(&shell, &args, command)
}

fn execute_with_default_shell(command: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        execute_shell_command("cmd.exe", &["/c"], command).value
    }
    #[cfg(not(target_os = "windows"))]
    {
        execute_shell_command("/bin/sh", &["-c"], command).value
    }
}

fn execute_shell_command(shell: &str, args: &[&str], command: &str) -> ShellResult {
    let rt = tokio::runtime::Handle::try_current();
    match rt {
        Ok(handle) => {
            let result = handle.block_on(async {
                let output = Command::new(shell)
                    .args(args)
                    .arg(command)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .kill_on_drop(true)
                    .output()
                    .await;

                match output {
                    Ok(out) if out.status.success() => {
                        let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        ShellResult {
                            executed: true,
                            value: if value.is_empty() { None } else { Some(value) },
                        }
                    }
                    Ok(_) => ShellResult { executed: true, value: None },
                    Err(_) => ShellResult { executed: false, value: None },
                }
            });
            result
        }
        Err(_) => {
            // No runtime available, fall back to sync execution
            let output = std::process::Command::new(shell)
                .args(args)
                .arg(command)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    ShellResult {
                        executed: true,
                        value: if value.is_empty() { None } else { Some(value) },
                    }
                }
                Ok(_) => ShellResult { executed: true, value: None },
                Err(_) => ShellResult { executed: false, value: None },
            }
        }
    }
}
