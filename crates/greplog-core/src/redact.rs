use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Available redaction strategies for sensitive data
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RedactionMode {
    /// Completely replace the value with `[REDACTED]`
    Full,
    /// Mask most of the string but keep first/last characters visible (e.g. `s***@example.com`)
    Partial,
    /// Replace the value with a one-way deterministic hash
    Hash,
}

/// Applies the specified redaction strategy to a string value
pub fn redact_string(val: &str, mode: RedactionMode) -> String {
    if val.is_empty() {
        return String::new();
    }

    match mode {
        RedactionMode::Full => "[REDACTED]".to_string(),
        RedactionMode::Partial => {
            let len = val.chars().count();
            if len <= 4 {
                // BUG FIX: Originally this returned `val.to_string()` directly, leaking short strings!
                return "[***]".to_string();
            }

            // Keep first 2 and last 2 characters visible
            let first: String = val.chars().take(2).collect();
            let last: String = val.chars().skip(len - 2).collect();
            format!("{}***{}", first, last)
        }
        RedactionMode::Hash => {
            let mut hasher = DefaultHasher::new();
            val.hash(&mut hasher);
            let hash = hasher.finish();
            format!("[HASH:{:016x}]", hash)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_full() {
        assert_eq!(redact_string("secret_password", RedactionMode::Full), "[REDACTED]");
        assert_eq!(redact_string("", RedactionMode::Full), "");
    }

    #[test]
    fn test_redact_partial() {
        // Standard strings
        assert_eq!(redact_string("password123", RedactionMode::Partial), "pa***23");
        assert_eq!(redact_string("hello", RedactionMode::Partial), "he***lo");

        // Short strings (must be fully masked to prevent leaking)
        assert_eq!(redact_string("abcd", RedactionMode::Partial), "[***]");
        assert_eq!(redact_string("hi", RedactionMode::Partial), "[***]");
        assert_eq!(redact_string("", RedactionMode::Partial), "");
    }

    #[test]
    fn test_redact_hash() {
        let h1 = redact_string("user@example.com", RedactionMode::Hash);
        let h2 = redact_string("user@example.com", RedactionMode::Hash);
        let h3 = redact_string("admin@example.com", RedactionMode::Hash);

        assert!(h1.starts_with("[HASH:"));
        assert!(h1.ends_with("]"));
        
        // Deterministic
        assert_eq!(h1, h2);
        // Collision resistant
        assert_ne!(h1, h3);
    }
}
