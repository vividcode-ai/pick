fn main() {
    let web_dist = std::path::Path::new("../../web/dist");
    if !web_dist.join("index.html").exists() {
        println!(
            "cargo:warning=Web SPA not found at {}. Running npm build...",
            web_dist.display()
        );
        let status = std::process::Command::new("npm")
            .args(["run", "build"])
            .current_dir("../../web")
            .status()
            .expect("Failed to run npm build");
        assert!(
            status.success(),
            "npm run build failed. Run 'cd web && npm install && npm run build' manually."
        );
    }
}
