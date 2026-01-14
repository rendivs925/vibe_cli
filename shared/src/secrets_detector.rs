use regex::Regex;

/// Secrets detection and protection system
pub struct SecretsDetector {
    patterns: Vec<SecretPattern>,
    compiled_regexes: Vec<Regex>,
}

#[derive(Debug, Clone)]
pub struct SecretPattern {
    pub name: String,
    pub regex: String,
    pub severity: SecretSeverity,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretSeverity {
    High,   // API keys, passwords, private keys
    Medium, // Tokens, session IDs, database credentials
    Low,    // Public keys, non-sensitive config
}

#[derive(Debug, Clone)]
pub struct SecretFinding {
    pub pattern_name: String,
    pub severity: SecretSeverity,
    pub line_number: Option<usize>,
    pub position: Option<usize>,
    pub description: String,
    pub masked_value: String,
}

/// Result of secrets scanning
#[derive(Debug, Clone)]
pub struct SecretsScanResult {
    pub findings: Vec<SecretFinding>,
    pub total_secrets_found: usize,
    pub high_severity_count: usize,
    pub medium_severity_count: usize,
    pub low_severity_count: usize,
    pub sanitized_content: String,
}

impl SecretsDetector {
    pub fn new() -> Self {
        let patterns = vec![
            // High severity patterns
            SecretPattern {
                name: "AWS Access Key ID".to_string(),
                regex: "AKIA[0-9A-Z]{16,}".to_string(),
                severity: SecretSeverity::High,
                description: "AWS Access Key ID detected".to_string(),
            },
            SecretPattern {
                name: "JWT Token".to_string(),
                regex: "eyJ[A-Za-z0-9_-]*\\.eyJ[A-Za-z0-9_-]*\\.[A-Za-z0-9_-]*".to_string(),
                severity: SecretSeverity::High,
                description: "JWT token detected".to_string(),
            },
            SecretPattern {
                name: "Private Key".to_string(),
                regex: "-----BEGIN\\s+(?:RSA\\s+)?PRIVATE\\s+KEY-----".to_string(),
                severity: SecretSeverity::High,
                description: "Private key detected".to_string(),
            },
            SecretPattern {
                name: "Database Connection String".to_string(),
                regex: "(?i)(mongodb|mysql|postgresql)://[^\\s'\"]+".to_string(),
                severity: SecretSeverity::High,
                description: "Database connection string detected".to_string(),
            },
            // Medium severity patterns
            SecretPattern {
                name: "GitHub Token".to_string(),
                regex: "ghp_[A-Za-z0-9]{20,}".to_string(),
                severity: SecretSeverity::Medium,
                description: "GitHub personal access token detected".to_string(),
            },
            SecretPattern {
                name: "Slack Token".to_string(),
                regex: "xoxb-[0-9]+-[0-9]+-[A-Za-z0-9]+".to_string(),
                severity: SecretSeverity::Medium,
                description: "Slack bot token detected".to_string(),
            },
            // Generic patterns
            SecretPattern {
                name: "Generic API Key".to_string(),
                regex:
                    "(?i)(api_key|apikey|secret_key|access_token|auth_token)[=:][A-Za-z0-9_-]{20,}"
                        .to_string(),
                severity: SecretSeverity::Medium,
                description: "Generic API key or token detected".to_string(),
            },
            SecretPattern {
                name: "Generic Password".to_string(),
                regex: "(?i)(password|passwd|pwd|pass)[=:][A-Za-z0-9!@#$%^&*()_+-=]{8,}"
                    .to_string(),
                severity: SecretSeverity::Medium,
                description: "Generic password detected".to_string(),
            },
            SecretPattern {
                name: "Stripe API Key".to_string(),
                regex: "sk[_-](?:test|live)?[_-]?[A-Za-z0-9]{10,}".to_string(),
                severity: SecretSeverity::High,
                description: "Stripe API key detected".to_string(),
            },
            // Low severity patterns (informational)
            SecretPattern {
                name: "SSH Public Key".to_string(),
                regex: "ssh-(?:rsa|dss|ed25519)\\s+[A-Za-z0-9+/]+[=]{0,3}".to_string(),
                severity: SecretSeverity::Low,
                description: "SSH public key detected (not sensitive)".to_string(),
            },
            SecretPattern {
                name: "Email Address".to_string(),
                regex: "[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}".to_string(),
                severity: SecretSeverity::Low,
                description: "Email address detected".to_string(),
            },
        ];

        let compiled_regexes = patterns
            .iter()
            .map(|pattern| Regex::new(&pattern.regex).unwrap_or_else(|_| Regex::new("").unwrap()))
            .collect();

        Self {
            patterns,
            compiled_regexes,
        }
    }

    /// Scan content for secrets
    pub fn scan_content(&self, content: &str) -> SecretsScanResult {
        let mut findings = Vec::new();
        let mut sanitized_content = content.to_string();

        // Find all secrets in original content
        for (pattern, regex) in self.patterns.iter().zip(&self.compiled_regexes) {
            for capture in regex.find_iter(content) {
                let matched_text = capture.as_str().to_string();
                let start = capture.start();

                let masked_value = self.mask_secret(&matched_text);
                findings.push(SecretFinding {
                    pattern_name: pattern.name.clone(),
                    severity: pattern.severity.clone(),
                    line_number: Some(content[..start].chars().filter(|&c| c == '\n').count() + 1),
                    position: Some(start),
                    description: pattern.description.clone(),
                    masked_value: masked_value.clone(),
                });

                // Replace in sanitized content
                let masked_value = self.mask_secret(&matched_text);
                sanitized_content = sanitized_content.replace(&matched_text, &masked_value);
            }
        }

        // Count by severity
        let mut high_count = 0;
        let mut medium_count = 0;
        let mut low_count = 0;

        for finding in &findings {
            match finding.severity {
                SecretSeverity::High => high_count += 1,
                SecretSeverity::Medium => medium_count += 1,
                SecretSeverity::Low => low_count += 1,
            }
        }

        let total_secrets = findings.len();
        SecretsScanResult {
            findings,
            total_secrets_found: total_secrets,
            high_severity_count: high_count,
            medium_severity_count: medium_count,
            low_severity_count: low_count,
            sanitized_content,
        }
    }

    /// Check if content contains high-severity secrets
    pub fn contains_high_severity_secrets(&self, content: &str) -> bool {
        for (pattern, regex) in self.patterns.iter().zip(&self.compiled_regexes) {
            if matches!(pattern.severity, SecretSeverity::High) && regex.is_match(content) {
                return true;
            }
        }
        false
    }

    /// Sanitize content by masking secrets
    pub fn sanitize_content(&self, content: &str) -> String {
        self.scan_content(content).sanitized_content
    }

    /// Mask a secret value for safe display
    fn mask_secret(&self, secret: &str) -> String {
        if secret.len() <= 8 {
            return "*".repeat(secret.len());
        }

        let visible_chars = 4;
        let masked_chars = secret.len().saturating_sub(visible_chars * 2);

        if masked_chars <= 0 {
            return "*".repeat(secret.len());
        }

        format!(
            "{}{}{}",
            &secret[..visible_chars],
            "*".repeat(masked_chars),
            &secret[secret.len().saturating_sub(visible_chars)..]
        )
    }

    /// Get security recommendations based on scan results
    pub fn get_security_recommendations(&self, result: &SecretsScanResult) -> Vec<String> {
        let mut recommendations = Vec::new();

        if result.high_severity_count > 0 {
            recommendations.push(format!(
                "🚨 CRITICAL: {} high-severity secrets detected. These must be removed immediately!",
                result.high_severity_count
            ));
            recommendations
                .push("   - Rotate any exposed API keys, passwords, or private keys".to_string());
            recommendations
                .push("   - Use environment variables or secure credential stores".to_string());
        }

        if result.medium_severity_count > 0 {
            recommendations.push(format!(
                "⚠️  WARNING: {} medium-severity secrets detected.",
                result.medium_severity_count
            ));
            recommendations
                .push("   - Review tokens and session IDs for exposure risks".to_string());
        }

        if result.low_severity_count > 0 {
            recommendations.push(format!(
                "ℹ️  INFO: {} low-severity items detected (may include public keys, emails).",
                result.low_severity_count
            ));
        }

        if result.total_secrets_found == 0 {
            recommendations.push("✅ No secrets detected in the scanned content.".to_string());
        }

        recommendations
    }
}

impl Default for SecretsDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_key_detection() {
        let detector = SecretsDetector::new();
        let content = "My AWS key is AKIAIOSFODNN7EXAMPLE and JWT is eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";

        let result = detector.scan_content(content);
        assert!(result.total_secrets_found >= 2);
        assert!(result.high_severity_count >= 2);
    }

    #[test]
    fn test_jwt_detection() {
        let detector = SecretsDetector::new();
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";

        let result = detector.scan_content(jwt);
        assert!(result.total_secrets_found >= 1);
    }

    #[test]
    fn test_secret_masking() {
        let detector = SecretsDetector::new();
        let secret = "AKIAIOSFODNN7EXAMPLE";

        let masked = detector.mask_secret(secret);
        assert_eq!(masked, "AKIA************MPLE");
        assert_eq!(masked.len(), secret.len());
    }

    #[test]
    fn test_content_sanitization() {
        let detector = SecretsDetector::new();
        let content = "API key: sk_test_1234567890abcdef password: mysecret123";

        let sanitized = detector.sanitize_content(content);
        assert!(sanitized.contains("sk_t****************cdef"));
        assert!(!sanitized.contains("sk_test_1234567890abcdef"));
    }
}
