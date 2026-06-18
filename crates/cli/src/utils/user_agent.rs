//! User agent string generation.

/// Get the user agent string for the application.
pub fn get_user_agent(version: &str) -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    format!("Pick-cli/{} ({}; rust; {})", version, os, arch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_agent_format() {
        let ua = get_user_agent("1.0.0");
        assert!(ua.starts_with("Pick-cli/1.0.0"));
        assert!(ua.contains("("));
        assert!(ua.contains(")"));
        assert!(ua.contains("rust"));
    }
}
