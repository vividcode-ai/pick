//! HTTP proxy configuration

use std::env;

const DEFAULT_PROXY_PORTS: &[(&str, u16)] = &[
    ("ftp", 21),
    ("gopher", 70),
    ("http", 80),
    ("https", 443),
    ("ws", 80),
    ("wss", 443),
];

/// Proxy configuration
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub no_proxy: Option<String>,
}

impl ProxyConfig {
    /// Create proxy configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            http_proxy: env::var("HTTP_PROXY")
                .or_else(|_| env::var("http_proxy"))
                .ok(),
            https_proxy: env::var("HTTPS_PROXY")
                .or_else(|_| env::var("https_proxy"))
                .ok(),
            no_proxy: env::var("NO_PROXY").or_else(|_| env::var("no_proxy")).ok(),
        }
    }

    /// Check if a URL should bypass the proxy
    pub fn should_bypass(&self, host: &str) -> bool {
        let no_proxy = match &self.no_proxy {
            Some(n) => n,
            None => return false,
        };
        if no_proxy.trim() == "*" {
            return true;
        }
        no_proxy.split(',').map(|s| s.trim()).any(|pattern| {
            if pattern.is_empty() {
                return false;
            }
            // Support host:port patterns
            let (pattern_host, pattern_port) = match pattern.split_once(':') {
                Some((h, p)) => (h, p.parse::<u16>().ok()),
                None => (pattern, None),
            };

            // Match hostname (with wildcard support)
            let host_match = if pattern_host.starts_with('*') {
                host.ends_with(&pattern_host[1..])
            } else if pattern_host.starts_with('.') {
                host.ends_with(pattern_host)
            } else {
                host == pattern_host
            };

            if !host_match {
                return false;
            }

            // If no port specified, match all ports for this host
            pattern_port.is_none()
        })
    }

    /// Get the proxy URL for a given protocol
    pub fn get_proxy_url(&self, protocol: &str) -> Option<&str> {
        match protocol {
            "http" => self.http_proxy.as_deref(),
            "https" => self.https_proxy.as_deref().or(self.http_proxy.as_deref()),
            _ => None,
        }
    }

    /// Get a proxy URL for a specific target URL, respecting NO_PROXY.
    /// Returns None if no proxy is configured or the target should be bypassed.
    pub fn get_proxy_for_url(&self, target_url: &str) -> Option<String> {
        let parsed = parse_proxy_target_url(target_url)?;
        let protocol = parsed.protocol.trim_end_matches(':');
        let hostname = parsed.hostname.clone();
        let port = parsed.port;

        if !should_proxy_hostname(&hostname, port, self.no_proxy.as_deref().unwrap_or("")) {
            return None;
        }

        // Try protocol-specific proxy first
        if let Some(proxy) = self.get_proxy_url(protocol) {
            if proxy.contains("://") {
                return Some(proxy.to_string());
            }
            return Some(format!("{}://{}", protocol, proxy));
        }

        // Fall back to all_proxy env var
        let all_proxy = env::var("ALL_PROXY")
            .or_else(|_| env::var("all_proxy"))
            .ok()?;
        if all_proxy.contains("://") {
            Some(all_proxy)
        } else {
            Some(format!("{}://{}", protocol, all_proxy))
        }
    }
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

/// Parsed target URL info
struct ParsedTargetUrl {
    protocol: String,
    hostname: String,
    port: u16,
}

fn parse_proxy_target_url(target_url: &str) -> Option<ParsedTargetUrl> {
    let url = if target_url.contains("://") {
        url::Url::parse(target_url).ok()?
    } else {
        url::Url::parse(&format!("https://{}", target_url)).ok()?
    };

    let protocol = url.scheme().to_string();
    let hostname = url.host_str()?.to_string();
    let port = url.port().unwrap_or_else(|| {
        DEFAULT_PROXY_PORTS
            .iter()
            .find(|(p, _)| *p == protocol)
            .map(|(_, port)| *port)
            .unwrap_or(0)
    });

    Some(ParsedTargetUrl {
        protocol,
        hostname,
        port,
    })
}

fn should_proxy_hostname(hostname: &str, port: u16, no_proxy: &str) -> bool {
    if no_proxy.is_empty() {
        return true;
    }
    if no_proxy.trim() == "*" {
        return false;
    }

    no_proxy
        .split([',', ' '])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .all(|proxy| {
            let (proxy_hostname, proxy_port) = match proxy.split_once(':') {
                Some((h, p)) => (h, p.parse::<u16>().ok()),
                None => (proxy, None),
            };

            // If port specified and doesn't match, not excluded
            if let Some(pp) = proxy_port
                && pp != port
            {
                return true;
            }

            let host_matches = if proxy_hostname.starts_with('*') {
                hostname.ends_with(&proxy_hostname[1..])
            } else if proxy_hostname.starts_with('.') {
                hostname.ends_with(proxy_hostname)
            } else {
                hostname == proxy_hostname
            };

            !host_matches
        })
}

/// Resolve proxy URL for a target URL.
/// Returns None if no proxy is configured or target should be bypassed.
pub fn resolve_http_proxy_url_for_target(target_url: &str) -> Option<String> {
    let config = ProxyConfig::from_env();
    config.get_proxy_for_url(target_url)
}

/// Unsupported proxy protocol error message
pub const UNSUPPORTED_PROXY_PROTOCOL_MESSAGE: &str = "Unsupported proxy protocol. SOCKS and PAC proxy URLs are not supported; use an HTTP or HTTPS proxy URL.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_bypass_exact_host() {
        let config = ProxyConfig {
            http_proxy: Some("http://proxy:8080".to_string()),
            https_proxy: None,
            no_proxy: Some("localhost,127.0.0.1".to_string()),
        };
        assert!(config.should_bypass("localhost"));
        assert!(config.should_bypass("127.0.0.1"));
        assert!(!config.should_bypass("example.com"));
    }

    #[test]
    fn test_should_bypass_wildcard() {
        let config = ProxyConfig {
            http_proxy: Some("http://proxy:8080".to_string()),
            https_proxy: None,
            no_proxy: Some("*".to_string()),
        };
        assert!(config.should_bypass("anything.com"));
    }

    #[test]
    fn test_should_bypass_wildcard_domain() {
        let config = ProxyConfig {
            http_proxy: Some("http://proxy:8080".to_string()),
            https_proxy: None,
            no_proxy: Some(".internal.com".to_string()),
        };
        assert!(config.should_bypass("app.internal.com"));
        assert!(!config.should_bypass("external.com"));
    }

    #[test]
    fn test_get_proxy_url() {
        let config = ProxyConfig {
            http_proxy: Some("http://proxy:8080".to_string()),
            https_proxy: Some("https://proxy:8443".to_string()),
            no_proxy: None,
        };
        assert_eq!(config.get_proxy_url("http"), Some("http://proxy:8080"));
        assert_eq!(config.get_proxy_url("https"), Some("https://proxy:8443"));
    }

    #[test]
    fn test_get_proxy_https_falls_back_to_http() {
        let config = ProxyConfig {
            http_proxy: Some("http://proxy:8080".to_string()),
            https_proxy: None,
            no_proxy: None,
        };
        assert_eq!(config.get_proxy_url("https"), Some("http://proxy:8080"));
    }

    #[test]
    fn test_no_proxy_empty() {
        let config = ProxyConfig {
            http_proxy: None,
            https_proxy: None,
            no_proxy: None,
        };
        assert_eq!(config.get_proxy_for_url("http://example.com"), None);
    }

    #[test]
    fn test_should_proxy_hostname_with_port() {
        let result = should_proxy_hostname("example.com", 443, "example.com:8080");
        assert!(result); // different port, should proxy
    }
}
