/// PII redaction rules shared across SDKs and agent.
///
/// Each rule defines a field pattern and a replacement strategy.
/// Both SDKs (pre-ship) and the agent (post-ingest) apply these.

#[derive(Debug, Clone)]
pub struct RedactRule {
    pub key_pattern: String,
    pub mode: RedactMode,
}

#[derive(Debug, Clone)]
pub enum RedactMode {
    /// Replace the entire value with `[REDACTED]`
    Mask,
    /// Keep first N chars, replace the rest
    Partial(usize),
    /// Replace with a hash of the value (one-way)
    Hash,
}

/// Default set of known sensitive keys.
pub fn default_rules() -> Vec<RedactRule> {
    vec![
        RedactRule { key_pattern: "password".into(), mode: RedactMode::Mask },
        RedactRule { key_pattern: "secret".into(), mode: RedactMode::Mask },
        RedactRule { key_pattern: "token".into(), mode: RedactMode::Mask },
        RedactRule { key_pattern: "api_key".into(), mode: RedactMode::Mask },
        RedactRule { key_pattern: "authorization".into(), mode: RedactMode::Mask },
        RedactRule { key_pattern: "credit_card".into(), mode: RedactMode::Partial(4) },
        RedactRule { key_pattern: "ssn".into(), mode: RedactMode::Mask },
        RedactRule { key_pattern: "email".into(), mode: RedactMode::Partial(3) },
    ]
}

/// Apply all matching rules to an attribute key-value pair.
/// Returns the (possibly redacted) value.
pub fn apply_rules(key: &str, value: &str, rules: &[RedactRule]) -> String {
    for rule in rules {
        if key.to_lowercase().contains(&rule.key_pattern.to_lowercase()) {
            return match rule.mode {
                RedactMode::Mask => "[REDACTED]".to_string(),
                RedactMode::Partial(keep) => {
                    if value.len() <= keep {
                        value.to_string()
                    } else {
                        let (visible, _) = value.split_at(keep);
                        format!("{}[REDACTED]", visible)
                    }
                }
                RedactMode::Hash => {
                    let hash = blake3::hash(value.as_bytes());
                    hash.to_hex()[..16].to_string()
                }
            };
        }
    }
    value.to_string()
}

/// Convenience: redact all attributes in a `map` in-place.
pub fn redact_attributes(
    attrs: &mut std::collections::HashMap<String, String>,
    rules: &[RedactRule],
) {
    for (k, v) in attrs.iter_mut() {
        *v = apply_rules(k, v, rules);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_password() {
        let rules = default_rules();
        assert_eq!(apply_rules("password", "hunter2", &rules), "[REDACTED]");
    }

    #[test]
    fn test_partial_credit_card() {
        let rules = default_rules();
        let result = apply_rules("credit_card", "4111111111111111", &rules);
        assert_eq!(result, "4111[REDACTED]");
    }

    #[test]
    fn test_short_value_partial_noop() {
        let rules = default_rules();
        assert_eq!(apply_rules("email", "a@b", &rules), "a@b");
    }

    #[test]
    fn test_no_match_passes_through() {
        let rules = default_rules();
        assert_eq!(apply_rules("my_field", "hello", &rules), "hello");
    }
}
