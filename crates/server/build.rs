fn main() {
    println!("cargo::rerun-if-changed=../../web/src");
    println!("cargo::rerun-if-changed=../../web/index.html");
    println!("cargo::rerun-if-changed=../../web/package.json");
    println!("cargo::rerun-if-changed=../../web/vite.config.ts");

    let web_dir = std::path::Path::new("../../web");

    // Clean previous build artifacts to avoid EPERM issues on Windows
    let dist = web_dir.join("dist");
    if dist.exists() {
        let _ = std::fs::remove_dir_all(&dist);
    }
    let nm = web_dir.join("node_modules");
    if nm.exists() {
        let _ = std::fs::remove_dir_all(&nm);
    }

    let (shell, flag) = if cfg!(windows) {
        ("cmd", "/c")
    } else {
        ("sh", "-c")
    };
    let status = std::process::Command::new(shell)
        .args([flag, "npm install && npm run build"])
        .current_dir(web_dir)
        .status()
        .expect("Failed to run npm build");
    assert!(status.success(), "npm build failed");
}
