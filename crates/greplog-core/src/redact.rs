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


