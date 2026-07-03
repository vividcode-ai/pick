fn main() {
    println!("cargo::rerun-if-changed=../../web/src");
    println!("cargo::rerun-if-changed=../../web/index.html");
    println!("cargo::rerun-if-changed=../../web/package.json");
    println!("cargo::rerun-if-changed=../../web/vite.config.ts");

    // Clean dist directory to avoid EPERM issues on Windows (locked files from previous builds)
    let dist = std::path::Path::new("../../web/dist");
    if dist.exists() {
        let _ = std::fs::remove_dir_all(dist);
    }

    let (shell, flag) = if cfg!(windows) {
        ("cmd", "/c")
    } else {
        ("sh", "-c")
    };
    let status = std::process::Command::new(shell)
        .args([flag, "npm run build"])
        .current_dir("../../web")
        .status()
        .expect("Failed to run npm build");
    assert!(
        status.success(),
        "npm run build failed. cd web && npm install && npm run build"
    );
}
