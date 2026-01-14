use std::collections::HashSet;
use url::Url;

/// Network security manager with default-deny policy
pub struct NetworkSecurity {
    allowed_domains: HashSet<String>,
    blocked_domains: HashSet<String>,
    allowed_schemes: HashSet<String>,
    max_request_size: usize,
    max_response_size: usize,
    request_timeout: std::time::Duration,
}

impl NetworkSecurity {
    pub fn new() -> Self {
        let mut allowed_domains = HashSet::new();
        let mut blocked_domains = HashSet::new();
        let mut allowed_schemes = HashSet::new();

        // Allowlist of safe domains for development/documentation
        allowed_domains.insert("docs.rust-lang.org".to_string());
        allowed_domains.insert("crates.io".to_string());
        allowed_domains.insert("doc.rust-lang.org".to_string());
        allowed_domains.insert("github.com".to_string()); // For repository access
        allowed_domains.insert("raw.githubusercontent.com".to_string()); // For raw file access

        // Blocklist of known malicious domains (can be expanded)
        blocked_domains.insert("malicious.example.com".to_string());
        blocked_domains.insert("evil.com".to_string());

        // Only allow HTTPS for security
        allowed_schemes.insert("https".to_string());

        Self {
            allowed_domains,
            blocked_domains,
            allowed_schemes,
            max_request_size: 1024,             // 1KB max request size
            max_response_size: 5 * 1024 * 1024, // 5MB max response size
            request_timeout: std::time::Duration::from_secs(30),
        }
    }

    /// Check if a URL is allowed for access
    pub fn is_url_allowed(&self, url_str: &str) -> Result<(), NetworkSecurityError> {
        let url = Url::parse(url_str)
            .map_err(|e| NetworkSecurityError::InvalidUrl(format!("Invalid URL: {}", e)))?;

        // Check scheme
        if !self.allowed_schemes.contains(url.scheme()) {
            return Err(NetworkSecurityError::ForbiddenScheme(
                url.scheme().to_string(),
            ));
        }

        // Check domain
        if let Some(domain) = url.host_str() {
            // Check blocklist first
            if self.blocked_domains.contains(domain) {
                return Err(NetworkSecurityError::BlockedDomain(domain.to_string()));
            }

            // Check allowlist
            if !self.allowed_domains.contains(domain) {
                return Err(NetworkSecurityError::DomainNotAllowed(domain.to_string()));
            }
        } else {
            return Err(NetworkSecurityError::NoHostInUrl);
        }

        Ok(())
    }

    /// Validate request size
    pub fn validate_request_size(&self, size: usize) -> Result<(), NetworkSecurityError> {
        if size > self.max_request_size {
            return Err(NetworkSecurityError::RequestTooLarge(
                size,
                self.max_request_size,
            ));
        }
        Ok(())
    }

    /// Validate response size
    pub fn validate_response_size(&self, size: usize) -> Result<(), NetworkSecurityError> {
        if size > self.max_response_size {
            return Err(NetworkSecurityError::ResponseTooLarge(
                size,
                self.max_response_size,
            ));
        }
        Ok(())
    }

    /// Get request timeout
    pub fn request_timeout(&self) -> std::time::Duration {
        self.request_timeout
    }

    /// Add a domain to the allowlist (admin function)
    pub fn allow_domain(&mut self, domain: String) {
        self.allowed_domains.insert(domain);
    }

    /// Remove a domain from the allowlist (admin function)
    pub fn deny_domain(&mut self, domain: &str) {
        self.allowed_domains.remove(domain);
    }

    /// Get list of allowed domains
    pub fn allowed_domains(&self) -> &HashSet<String> {
        &self.allowed_domains
    }
}

#[derive(Debug, Clone)]
pub enum NetworkSecurityError {
    InvalidUrl(String),
    ForbiddenScheme(String),
    BlockedDomain(String),
    DomainNotAllowed(String),
    NoHostInUrl,
    RequestTooLarge(usize, usize),
    ResponseTooLarge(usize, usize),
    RequestTimeout,
}

impl std::fmt::Display for NetworkSecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkSecurityError::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
            NetworkSecurityError::ForbiddenScheme(scheme) => {
                write!(f, "Forbidden URL scheme: {}", scheme)
            }
            NetworkSecurityError::BlockedDomain(domain) => write!(f, "Domain blocked: {}", domain),
            NetworkSecurityError::DomainNotAllowed(domain) => {
                write!(f, "Domain not in allowlist: {}", domain)
            }
            NetworkSecurityError::NoHostInUrl => write!(f, "URL has no host"),
            NetworkSecurityError::RequestTooLarge(size, limit) => {
                write!(f, "Request too large: {} > {} bytes", size, limit)
            }
            NetworkSecurityError::ResponseTooLarge(size, limit) => {
                write!(f, "Response too large: {} > {} bytes", size, limit)
            }
            NetworkSecurityError::RequestTimeout => write!(f, "Request timeout"),
        }
    }
}

impl std::error::Error for NetworkSecurityError {}

impl Default for NetworkSecurity {
    fn default() -> Self {
        Self::new()
    }
}

/// Secure HTTP client wrapper with network security
pub struct SecureHttpClient {
    client: reqwest::Client,
    security: NetworkSecurity,
}

impl SecureHttpClient {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let security = NetworkSecurity::new();

        let client = reqwest::Client::builder()
            .user_agent("VibeCLI/1.0 (secure)")
            .timeout(security.request_timeout())
            .https_only(true) // Force HTTPS only
            .build()?;

        Ok(Self { client, security })
    }

    /// Make a secure GET request
    pub async fn get(&self, url: &str) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
        // Security check
        self.security.is_url_allowed(url)?;

        // Make request
        let response = self.client.get(url).send().await?;

        // Check response size
        if let Some(content_length) = response.content_length() {
            self.security
                .validate_response_size(content_length as usize)?;
        }

        Ok(response)
    }

    /// Make a secure POST request
    pub async fn post(
        &self,
        url: &str,
        body: &str,
    ) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
        // Security checks
        self.security.is_url_allowed(url)?;
        self.security.validate_request_size(body.len())?;

        // Make request
        let response = self.client.post(url).body(body.to_string()).send().await?;

        // Check response size
        if let Some(content_length) = response.content_length() {
            self.security
                .validate_response_size(content_length as usize)?;
        }

        Ok(response)
    }

    /// Get security manager for configuration
    pub fn security(&mut self) -> &mut NetworkSecurity {
        &mut self.security
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_domains() {
        let security = NetworkSecurity::new();

        // Should allow docs.rust-lang.org
        assert!(security
            .is_url_allowed("https://docs.rust-lang.org/std/")
            .is_ok());

        // Should allow crates.io
        assert!(security
            .is_url_allowed("https://crates.io/crates/serde")
            .is_ok());

        // Should block HTTP
        assert!(security
            .is_url_allowed("http://docs.rust-lang.org/std/")
            .is_err());

        // Should block unlisted domains
        assert!(security.is_url_allowed("https://example.com/").is_err());
    }

    #[test]
    fn test_domain_management() {
        let mut security = NetworkSecurity::new();

        // Add domain
        security.allow_domain("example.com".to_string());
        assert!(security.is_url_allowed("https://example.com/").is_ok());

        // Remove domain
        security.deny_domain("example.com");
        assert!(security.is_url_allowed("https://example.com/").is_err());
    }
}
