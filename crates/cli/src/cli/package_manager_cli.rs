//! Package manager CLI commands

/// Run a package manager CLI command
pub async fn run_package_manager_cli(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        print_help();
        return Ok(());
    }

    match args[0].as_str() {
        "install" | "i" => {
            if args.len() < 2 {
                return Err("Usage: pkg install <package>".to_string());
            }
            let package = &args[1];
            println!("Installing package: {}...", package);
            let result = tokio::process::Command::new("npm")
                .arg("install")
                .arg(package)
                .output()
                .await
                .map_err(|e| format!("Failed to run npm install: {}", e))?;
            if result.status.success() {
                println!("Package installed successfully.");
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                eprintln!("Installation failed: {}", stderr);
            }
        }
        "remove" | "uninstall" | "r" => {
            if args.len() < 2 {
                return Err("Usage: pkg remove <package>".to_string());
            }
            let package = &args[1];
            println!("Removing package: {}...", package);
            let result = tokio::process::Command::new("npm")
                .arg("uninstall")
                .arg(package)
                .output()
                .await
                .map_err(|e| format!("Failed to run npm uninstall: {}", e))?;
            if result.status.success() {
                println!("Package removed successfully.");
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                eprintln!("Removal failed: {}", stderr);
            }
        }
        "list" | "ls" => {
            let result = tokio::process::Command::new("npm")
                .arg("list")
                .arg("--depth=0")
                .output()
                .await
                .map_err(|e| format!("Failed to list packages: {}", e))?;
            let stdout = String::from_utf8_lossy(&result.stdout);
            println!("{}", stdout);
        }
        "help" | "--help" | "-h" => {
            print_help();
        }
        _ => {
            return Err(format!(
                "Unknown command: {}. Use 'pkg help' for usage.",
                args[0]
            ));
        }
    }

    Ok(())
}

fn print_help() {
    println!("Package Manager CLI");
    println!("Usage: pkg <command> [args]");
    println!();
    println!("Commands:");
    println!("  install, i <package>    Install a package");
    println!("  remove, uninstall, r <package>  Remove a package");
    println!("  list, ls               List installed packages");
    println!("  help                   Show this help");
}
