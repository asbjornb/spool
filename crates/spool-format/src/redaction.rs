//! Secret detection and redaction utilities.
//!
//! Redaction in Spool is DESTRUCTIVE - secrets are replaced before export,
//! never stored in the output file.

use regex::Regex;
use std::borrow::Cow;

/// A detected secret in text.
#[derive(Debug, Clone)]
pub struct DetectedSecret {
    /// Start byte offset
    pub start: usize,
    /// End byte offset
    pub end: usize,
    /// The category of secret
    pub reason: SecretCategory,
    /// The matched text (for confirmation UI)
    pub matched: String,
}

/// Categories of secrets we detect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretCategory {
    ApiKey,
    Password,
    Email,
    Phone,
    IpAddress,
    PrivateKey,
    AwsKey,
    GitHubToken,
    JwtToken,
}

impl SecretCategory {
    /// Get the replacement text for this category.
    pub fn replacement(&self) -> &'static str {
        match self {
            SecretCategory::ApiKey => "[REDACTED:api_key]",
            SecretCategory::Password => "[REDACTED:password]",
            SecretCategory::Email => "[REDACTED:email]",
            SecretCategory::Phone => "[REDACTED:phone]",
            SecretCategory::IpAddress => "[REDACTED:ip_address]",
            SecretCategory::PrivateKey => "[REDACTED:private_key]",
            SecretCategory::AwsKey => "[REDACTED:aws_key]",
            SecretCategory::GitHubToken => "[REDACTED:github_token]",
            SecretCategory::JwtToken => "[REDACTED:jwt_token]",
        }
    }
}

/// Configuration for the secret detector.
#[derive(Debug, Clone)]
pub struct RedactionConfig {
    /// Detect API keys (generic patterns)
    pub detect_api_keys: bool,
    /// Detect email addresses
    pub detect_emails: bool,
    /// Detect phone numbers
    pub detect_phones: bool,
    /// Detect IP addresses
    pub detect_ip_addresses: bool,
    /// Detect private keys (PEM format)
    pub detect_private_keys: bool,
    /// Detect AWS access keys
    pub detect_aws_keys: bool,
    /// Detect GitHub tokens
    pub detect_github_tokens: bool,
    /// Detect JWT tokens
    pub detect_jwt_tokens: bool,
    /// Custom patterns to detect
    pub custom_patterns: Vec<(String, String)>, // (pattern, replacement)
}

impl Default for RedactionConfig {
    fn default() -> Self {
        Self {
            detect_api_keys: true,
            detect_emails: true,
            detect_phones: true,
            detect_ip_addresses: true,
            detect_private_keys: true,
            detect_aws_keys: true,
            detect_github_tokens: true,
            detect_jwt_tokens: true,
            custom_patterns: Vec::new(),
        }
    }
}

/// Detects secrets in text.
pub struct SecretDetector {
    config: RedactionConfig,
    patterns: Vec<(Regex, SecretCategory)>,
}

impl SecretDetector {
    /// Create a new detector with the given config.
    pub fn new(config: RedactionConfig) -> Self {
        let mut patterns = Vec::new();

        if config.detect_api_keys {
            // Generic API key patterns
            // sk-ant-api03-... (Anthropic)
            patterns.push((
                Regex::new(r"sk-ant-api\d{2}-[a-zA-Z0-9_-]{40,}").unwrap(),
                SecretCategory::ApiKey,
            ));
            // sk-... (OpenAI)
            patterns.push((
                Regex::new(r"sk-[a-zA-Z0-9]{32,}").unwrap(),
                SecretCategory::ApiKey,
            ));
            // Generic "key" followed by long string
            patterns.push((
                Regex::new(r#"['"](api[_-]?)?key['"]?\s*[:=]\s*['"][a-zA-Z0-9_-]{20,}['"]"#).unwrap(),
                SecretCategory::ApiKey,
            ));
        }

        if config.detect_emails {
            patterns.push((
                Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap(),
                SecretCategory::Email,
            ));
        }

        if config.detect_phones {
            // US phone numbers
            patterns.push((
                Regex::new(r"\b\d{3}[-.]?\d{3}[-.]?\d{4}\b").unwrap(),
                SecretCategory::Phone,
            ));
            // International format
            patterns.push((
                Regex::new(r"\+\d{1,3}[-.\s]?\d{1,14}").unwrap(),
                SecretCategory::Phone,
            ));
        }

        if config.detect_ip_addresses {
            // IPv4
            patterns.push((
                Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap(),
                SecretCategory::IpAddress,
            ));
        }

        if config.detect_private_keys {
            patterns.push((
                Regex::new(r"-----BEGIN [A-Z ]+ PRIVATE KEY-----").unwrap(),
                SecretCategory::PrivateKey,
            ));
        }

        if config.detect_aws_keys {
            patterns.push((
                Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
                SecretCategory::AwsKey,
            ));
        }

        if config.detect_github_tokens {
            patterns.push((
                Regex::new(r"ghp_[a-zA-Z0-9]{36}").unwrap(),
                SecretCategory::GitHubToken,
            ));
            patterns.push((
                Regex::new(r"github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}").unwrap(),
                SecretCategory::GitHubToken,
            ));
        }

        if config.detect_jwt_tokens {
            patterns.push((
                Regex::new(r"eyJ[a-zA-Z0-9_-]+\.eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+").unwrap(),
                SecretCategory::JwtToken,
            ));
        }

        Self { config, patterns }
    }

    /// Create a detector with default config.
    pub fn with_defaults() -> Self {
        Self::new(RedactionConfig::default())
    }

    /// Detect all secrets in the given text.
    pub fn detect(&self, text: &str) -> Vec<DetectedSecret> {
        let mut secrets = Vec::new();

        for (pattern, category) in &self.patterns {
            for m in pattern.find_iter(text) {
                secrets.push(DetectedSecret {
                    start: m.start(),
                    end: m.end(),
                    reason: *category,
                    matched: m.as_str().to_string(),
                });
            }
        }

        // Sort by start position and deduplicate overlapping matches
        secrets.sort_by_key(|s| s.start);
        deduplicate_overlapping(&mut secrets);

        secrets
    }

    /// Redact all detected secrets in the text, returning the redacted text.
    pub fn redact(&self, text: &str) -> (String, Vec<DetectedSecret>) {
        let secrets = self.detect(text);

        if secrets.is_empty() {
            return (text.to_string(), secrets);
        }

        let mut result = String::with_capacity(text.len());
        let mut last_end = 0;

        for secret in &secrets {
            // Add text before the secret
            result.push_str(&text[last_end..secret.start]);
            // Add replacement
            result.push_str(secret.reason.replacement());
            last_end = secret.end;
        }

        // Add remaining text
        result.push_str(&text[last_end..]);

        (result, secrets)
    }
}

/// Remove overlapping matches, keeping the longer one.
fn deduplicate_overlapping(secrets: &mut Vec<DetectedSecret>) {
    let mut i = 0;
    while i + 1 < secrets.len() {
        if secrets[i].end > secrets[i + 1].start {
            // Overlapping - keep the longer one
            if secrets[i].end - secrets[i].start >= secrets[i + 1].end - secrets[i + 1].start {
                secrets.remove(i + 1);
            } else {
                secrets.remove(i);
            }
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_anthropic_api_key() {
        let detector = SecretDetector::with_defaults();
        let text = "Using key: sk-ant-api03-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let secrets = detector.detect(text);
        assert_eq!(secrets.len(), 1);
        assert_eq!(secrets[0].reason, SecretCategory::ApiKey);
    }

    #[test]
    fn test_detect_email() {
        let detector = SecretDetector::with_defaults();
        let text = "Contact me at test@example.com for more info";
        let secrets = detector.detect(text);
        assert_eq!(secrets.len(), 1);
        assert_eq!(secrets[0].reason, SecretCategory::Email);
        assert_eq!(secrets[0].matched, "test@example.com");
    }

    #[test]
    fn test_redact_multiple() {
        let detector = SecretDetector::with_defaults();
        let text = "Email: test@example.com, Key: sk-ant-api03-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let (redacted, secrets) = detector.redact(text);
        assert_eq!(secrets.len(), 2);
        assert!(redacted.contains("[REDACTED:email]"));
        assert!(redacted.contains("[REDACTED:api_key]"));
        assert!(!redacted.contains("test@example.com"));
    }

    #[test]
    fn test_no_secrets() {
        let detector = SecretDetector::with_defaults();
        let text = "This is just regular text with no secrets.";
        let secrets = detector.detect(text);
        assert!(secrets.is_empty());
    }

    #[test]
    fn test_detect_github_token() {
        let detector = SecretDetector::with_defaults();
        let text = "Token: ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let secrets = detector.detect(text);
        assert_eq!(secrets.len(), 1);
        assert_eq!(secrets[0].reason, SecretCategory::GitHubToken);
    }

    #[test]
    fn test_detect_jwt() {
        let detector = SecretDetector::with_defaults();
        let text = "JWT: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let secrets = detector.detect(text);
        assert_eq!(secrets.len(), 1);
        assert_eq!(secrets[0].reason, SecretCategory::JwtToken);
    }
}
