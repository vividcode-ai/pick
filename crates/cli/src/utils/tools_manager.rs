//! External tools manager (fd, ripgrep auto-download)

use std::path::{Path, PathBuf};
use std::time::Duration;

const NETWORK_TIMEOUT_MS: u64 = 10_000;
const DOWNLOAD_TIMEOUT_MS: u64 = 120_000;

struct ToolCfg {
    name: &'static str,
    repo: &'static str,
    binary_name: &'static str,
    system_binary_names: &'static [&'static str],
    tag_prefix: &'static str,
}

fn get_tool_config(tool: &str) -> Option<ToolCfg> {
    match tool {
        "fd" => Some(ToolCfg {
            name: "fd",
            repo: "sharkdp/fd",
            binary_name: "fd",
            system_binary_names: &["fd", "fdfind"],
            tag_prefix: "v",
        }),
        "rg" => Some(ToolCfg {
            name: "ripgrep",
            repo: "BurntSushi/ripgrep",
            binary_name: "rg",
            system_binary_names: &["rg"],
            tag_prefix: "",
        }),
        _ => None,
    }
}

fn get_bin_dir() -> PathBuf {
    crate::config::get_agent_dir().join("bin")
}

fn binary_ext() -> &'static str {
    if cfg!(windows) { ".exe" } else { "" }
}

fn is_offline_mode() -> bool {
    std::env::var("PICK_OFFLINE").ok().is_some_and(|v| {
        v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
    })
}

/// Check if a command exists in PATH
fn command_exists(cmd: &str) -> bool {
    std::process::Command::new(cmd)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Get the path to a tool (system-wide or in our tools dir)
pub fn get_tool_path(tool: &str) -> Option<String> {
    let config = get_tool_config(tool)?;

    let local_path = get_bin_dir().join(format!("{}{}", config.binary_name, binary_ext()));
    if local_path.exists() {
        return Some(local_path.to_string_lossy().to_string());
    }

    for name in config.system_binary_names {
        if command_exists(name) {
            return Some(name.to_string());
        }
    }

    None
}

/// Ensure a tool is available, downloading if necessary
pub async fn ensure_tool(tool: &str, silent: bool) -> Option<String> {
    if let Some(path) = get_tool_path(tool) {
        return Some(path);
    }

    let config = get_tool_config(tool)?;

    if is_offline_mode() {
        if !silent {
            eprintln!(
                "{} not found. Offline mode enabled, skipping download.",
                config.name
            );
        }
        return None;
    }

    if !silent {
        eprintln!("{} not found. Downloading...", config.name);
    }

    match download_tool(tool, &config).await {
        Ok(path) => {
            if !silent {
                eprintln!("{} installed to {}", config.name, path.display());
            }
            Some(path.to_string_lossy().to_string())
        }
        Err(e) => {
            if !silent {
                eprintln!("Failed to download {}: {}", config.name, e);
            }
            None
        }
    }
}

fn os_arch_string() -> (&'static str, &'static str) {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let plat = match os {
        "macos" => "darwin",
        "linux" => "linux",
        "windows" => "win32",
        _ => os,
    };
    let arch_str = match arch {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        _ => arch,
    };
    (plat, arch_str)
}

fn get_asset_name(tool: &str, _config: &ToolCfg) -> Option<String> {
    let (plat, arch_str) = os_arch_string();
    match (tool, plat, arch_str) {
        ("fd", "darwin", a) => Some(format!("fd-v{{version}}-{a}-apple-darwin.tar.gz")),
        ("fd", "linux", a) => Some(format!("fd-v{{version}}-{a}-unknown-linux-gnu.tar.gz")),
        ("fd", "win32", a) => Some(format!("fd-v{{version}}-{a}-pc-windows-msvc.zip")),
        ("rg", "darwin", a) => Some(format!("ripgrep-{{version}}-{a}-apple-darwin.tar.gz")),
        ("rg", "linux", "x86_64") => Some(format!(
            "ripgrep-{{version}}-x86_64-unknown-linux-musl.tar.gz"
        )),
        ("rg", "linux", "arm64") => Some(format!(
            "ripgrep-{{version}}-aarch64-unknown-linux-gnu.tar.gz"
        )),
        ("rg", "win32", a) => Some(format!("ripgrep-{{version}}-{a}-pc-windows-msvc.zip")),
        _ => None,
    }
}

async fn get_latest_version(repo: &str) -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(NETWORK_TIMEOUT_MS))
        .user_agent("Pick-coding-agent")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("GitHub API error: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API error: {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    let tag = data["tag_name"].as_str().ok_or("Missing tag_name")?;
    Ok(tag.trim_start_matches('v').to_string())
}

async fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(DOWNLOAD_TIMEOUT_MS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("Download failed: {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Read failed: {}", e))?;
    std::fs::write(dest, &bytes).map_err(|e| format!("Write failed: {}", e))?;
    Ok(())
}

fn run_extraction(cmd: &str, args: &[&str]) -> Result<(), String> {
    let result = std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("Failed to run {}: {}", cmd, e))?;
    if result.success() {
        Ok(())
    } else {
        Err(format!("{} exited with {}", cmd, result))
    }
}

fn extract_tar_gz(archive: &Path, extract_dir: &Path) -> Result<(), String> {
    run_extraction(
        "tar",
        &[
            "xzf",
            &archive.to_string_lossy(),
            "-C",
            &extract_dir.to_string_lossy(),
        ],
    )
}

fn extract_zip(archive: &Path, extract_dir: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        // Try tar.exe from System32 (Windows ships bsdtar which handles zip)
        let system_root = std::env::var("SystemRoot")
            .or_else(|_| std::env::var("WINDIR"))
            .unwrap_or_default();
        let system_tar = Path::new(&system_root).join("System32").join("tar.exe");
        if system_tar.exists() {
            if run_extraction(
                &system_tar.to_string_lossy(),
                &[
                    "xf",
                    &archive.to_string_lossy(),
                    "-C",
                    &extract_dir.to_string_lossy(),
                ],
            )
            .is_ok()
            {
                return Ok(());
            }
        }
        // Fallback: PowerShell Expand-Archive
        let script = "& { param($a,$d) $ErrorActionPreference='Stop'; Expand-Archive -LiteralPath $a -DestinationPath $d -Force }";
        run_extraction(
            "powershell.exe",
            &[
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
                &archive.to_string_lossy(),
                &extract_dir.to_string_lossy(),
            ],
        )
    }
    #[cfg(not(windows))]
    {
        run_extraction(
            "unzip",
            &[
                "-q",
                &archive.to_string_lossy(),
                "-d",
                &extract_dir.to_string_lossy(),
            ],
        )
        .or_else(|_| {
            run_extraction(
                "tar",
                &[
                    "xf",
                    &archive.to_string_lossy(),
                    "-C",
                    &extract_dir.to_string_lossy(),
                ],
            )
        })
    }
}

fn find_binary(root: &Path, binary_name: &str) -> Option<PathBuf> {
    if root.is_dir() {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some(binary_name)
                {
                    return Some(path);
                }
                if path.is_dir() {
                    if let found @ Some(_) = find_binary(&path, binary_name) {
                        return found;
                    }
                }
            }
        }
    }
    None
}

async fn download_tool(tool: &str, config: &ToolCfg) -> Result<PathBuf, String> {
    let (plat, _arch_str) = os_arch_string();

    let version = get_latest_version(config.repo).await?;

    // Pin fd 10.3.0 on macOS x64
    let version = if tool == "fd" && plat == "darwin" && std::env::consts::ARCH == "x86_64" {
        "10.3.0".to_string()
    } else {
        version
    };

    let asset_name = get_asset_name(tool, config)
        .map(|s| s.replace("{version}", &version))
        .ok_or_else(|| format!("Unsupported platform: {}/{}", plat, std::env::consts::ARCH))?;

    let bin_dir = get_bin_dir();
    std::fs::create_dir_all(&bin_dir).map_err(|e| format!("Failed to create dir: {}", e))?;

    let download_url = format!(
        "https://github.com/{}/releases/download/{}{}/{}",
        config.repo, config.tag_prefix, version, asset_name
    );
    let archive_path = bin_dir.join(&asset_name);
    download_file(&download_url, &archive_path).await?;

    let binary_name = format!("{}{}", config.binary_name, binary_ext());
    let binary_path = bin_dir.join(&binary_name);

    // Extract to a unique temp dir to avoid races
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let extract_dir = bin_dir.join(format!(
        "extract_tmp_{}_{}_{}",
        config.binary_name,
        std::process::id(),
        timestamp
    ));
    std::fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("Failed to create extract dir: {}", e))?;

    let result = (|| -> Result<(), String> {
        if asset_name.ends_with(".tar.gz") {
            extract_tar_gz(&archive_path, &extract_dir)?;
        } else if asset_name.ends_with(".zip") {
            extract_zip(&archive_path, &extract_dir)?;
        } else {
            return Err(format!("Unsupported archive: {}", asset_name));
        }

        // Try common extraction paths
        let stripped_name = asset_name.replace(".tar.gz", "").replace(".zip", "");
        let candidates = [
            extract_dir.join(&stripped_name).join(&binary_name),
            extract_dir.join(&binary_name),
        ];
        let found = candidates.iter().find(|p| p.exists());
        let found = found
            .map(|p| p.clone())
            .or_else(|| find_binary(&extract_dir, &binary_name));

        match found {
            Some(src) => {
                std::fs::rename(&src, &binary_path).map_err(|e| format!("Move failed: {}", e))?;
                #[cfg(not(windows))]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&binary_path, std::fs::Permissions::from_mode(0o755))
                        .map_err(|e| format!("Chmod failed: {}", e))?;
                }
                Ok(())
            }
            None => Err(format!("Binary '{}' not found in archive", binary_name)),
        }
    })();

    // Cleanup
    let _ = std::fs::remove_file(&archive_path);
    let _ = std::fs::remove_dir_all(&extract_dir);

    result.map(|_| binary_path)
}
