use std::collections::HashSet;
use std::net::IpAddr;

#[derive(Debug, Clone)]
pub struct NetworkPolicy {
    pub blocked_domains: HashSet<String>,
    pub blocked_cidrs: Vec<(u32, u32)>, // (network_bits, mask_bits) for IPv4 CIDR matching
    pub allowed_domains: Option<HashSet<String>>,
    pub allow_all: bool,
    pub block_all: bool,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            blocked_domains: HashSet::new(),
            blocked_cidrs: Vec::new(),
            allowed_domains: None,
            allow_all: false,
            block_all: false,
        }
    }
}

impl NetworkPolicy {
    pub fn new_full_access() -> Self {
        Self {
            allow_all: true,
            block_all: false,
            ..Default::default()
        }
    }

    pub fn new_blocked() -> Self {
        Self {
            allow_all: false,
            block_all: true,
            ..Default::default()
        }
    }

    pub fn new_restricted() -> Self {
        let blocked_raw: [&str; 9] = [
            "localhost:*",
            "127.0.0.1:*",
            "169.254.0.0/16",
            "10.0.0.0/8",
            "172.16.0.0/12",
            "192.168.0.0/16",
            "100.64.0.0/10",
            "*.local",
            "*.internal",
        ];

        let mut blocked_domains = HashSet::new();
        let mut blocked_cidrs = Vec::new();
        for raw in &blocked_raw {
            if raw.contains('/') && !raw.contains(':') {
                // Parse as CIDR for efficient matching
                if let Some((network, mask)) = parse_ipv4_cidr(raw) {
                    blocked_cidrs.push((network, mask));
                    continue;
                }
            }
            blocked_domains.insert(raw.to_string());
        }

        Self {
            blocked_domains,
            blocked_cidrs,
            allow_all: false,
            block_all: false,
            allowed_domains: Some(
                ["api.anthropic.com", "api.openai.com", "*.googleapis.com"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
            ),
        }
    }

    pub fn can_access(&self, url: &str) -> Result<(), String> {
        if self.allow_all {
            return Ok(());
        }
        if self.block_all {
            return Err(format!("Network access is blocked: '{}'", url));
        }

        let domain = extract_domain(url);

        // First check blocklist — if blocked, reject immediately
        if self.is_blocked(&domain) {
            return Err(format!(
                "Network access denied: '{}' is blocked by policy",
                url
            ));
        }

        // Then check allowlist — if present, domain must be explicitly allowed
        if let Some(ref allowed) = self.allowed_domains {
            let is_allowed = allowed.iter().any(|a| domain_matches(&domain, a));
            if !is_allowed {
                return Err(format!(
                    "Network access denied: '{}' is not in the allowed domains list",
                    url
                ));
            }
        }

        Ok(())
    }

    fn is_blocked(&self, domain: &str) -> bool {
        // Check exact/wildcard domain blocklist
        for blocked in &self.blocked_domains {
            if domain_matches(domain, blocked) {
                return true;
            }
        }

        // Check CIDR blocklist (only for IP addresses)
        if let Ok(ip) = domain.parse::<IpAddr>() {
            if let IpAddr::V4(ipv4) = ip {
                let ip_bits = u32::from(ipv4);
                for &(network, mask) in &self.blocked_cidrs {
                    if mask == 0 || (ip_bits & (!0u32 << (32 - mask))) == network {
                        return true;
                    }
                }
            }
        }

        false
    }
}

/// Parse an IPv4 CIDR string like "10.0.0.0/8" into (network_bits, mask_bits).
fn parse_ipv4_cidr(cidr: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let mask: u32 = parts[1].parse().ok()?;
    if mask > 32 {
        return None;
    }
    let ip: std::net::Ipv4Addr = parts[0].parse().ok()?;
    let network = u32::from(ip) & (!0u32 << (32 - mask));
    Some((network, mask))
}

fn extract_domain(url: &str) -> String {
    let url = url.trim();
    if let Ok(parsed) = url::Url::parse(url) {
        parsed.host_str().unwrap_or("").to_lowercase()
    } else {
        // Try prepending https://
        if let Ok(parsed) = url::Url::parse(&format!("https://{}", url)) {
            parsed.host_str().unwrap_or("").to_lowercase()
        } else {
            url.to_lowercase()
        }
    }
}

fn domain_matches(domain: &str, pattern: &str) -> bool {
    // Exact match
    if pattern == domain {
        return true;
    }

    // Wildcard: *.example.com
    if let Some(suffix) = pattern.strip_prefix("*.") {
        return domain == suffix || domain.ends_with(&format!(".{}", suffix));
    }

    // IP:port with :* suffix: e.g. "127.0.0.1:*" matches "127.0.0.1", "127.0.0.1:8080"
    if let Some(host) = pattern.strip_suffix(":*") {
        return domain == host || domain.starts_with(&format!("{}:", host));
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_domain() {
        assert_eq!(
            extract_domain("https://api.anthropic.com/v1"),
            "api.anthropic.com"
        );
        assert_eq!(extract_domain("http://example.com/path"), "example.com");
        assert_eq!(extract_domain("example.com"), "example.com");
    }

    #[test]
    fn test_domain_matches_exact() {
        assert!(domain_matches("api.anthropic.com", "api.anthropic.com"));
        assert!(!domain_matches("api.anthropic.com", "api.openai.com"));
    }

    #[test]
    fn test_domain_matches_wildcard() {
        assert!(domain_matches("api.anthropic.com", "*.anthropic.com"));
        assert!(domain_matches("sub.api.anthropic.com", "*.anthropic.com"));
        assert!(!domain_matches("evil.com", "*.anthropic.com"));
    }

    #[test]
    fn test_full_access() {
        let policy = NetworkPolicy::new_full_access();
        assert!(policy.can_access("https://evil.com/malware").is_ok());
    }

    #[test]
    fn test_blocked_access() {
        let policy = NetworkPolicy::new_blocked();
        assert!(policy.can_access("https://example.com").is_err());
    }

    #[test]
    fn test_restricted_allows_known() {
        let policy = NetworkPolicy::new_restricted();
        assert!(
            policy
                .can_access("https://api.anthropic.com/v1/messages")
                .is_ok()
        );
    }

    #[test]
    fn test_restricted_blocks_internal() {
        let policy = NetworkPolicy::new_restricted();
        assert!(policy.can_access("http://localhost:8080").is_err());
        assert!(policy.can_access("http://192.168.1.1").is_err());
    }
}
