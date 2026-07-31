use greplog_core::redact::{redact_string, RedactionMode};

#[test]
fn test_redact_full() {
    assert_eq!(
        redact_string("secret_password", RedactionMode::Full),
        "[REDACTED]"
    );
    assert_eq!(redact_string("", RedactionMode::Full), "");
}

#[test]
fn test_redact_partial() {
    assert_eq!(
        redact_string("password123", RedactionMode::Partial),
        "pa***23"
    );
    assert_eq!(redact_string("hello", RedactionMode::Partial), "he***lo");

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

    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}
