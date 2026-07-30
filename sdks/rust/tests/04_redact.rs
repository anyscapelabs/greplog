mod common;

use std::thread;
use std::time::Duration;

#[test]
fn test_redaction_applies() {
    let (port, rx) = common::start_test_server();

    std::env::set_var("GREPLOG_TEST_PORT", port.to_string());
    greplog::init();

    thread::sleep(Duration::from_millis(200));

    greplog::info!("login", &[
        ("password", "secret123"),
        ("email", "user@example.com"),
        ("token", "abc-def-ghi"),
        ("username", "john"),
    ]);

    thread::sleep(Duration::from_millis(500));
    let batches = common::wait_for_frames(&rx, 1);

    assert!(!batches.is_empty(), "expected a batch");
    let batch = &batches[0];
    let log = &batch.logs[0];

    assert_eq!(log.attributes.get("password").map(|s| s.as_str()), Some("[REDACTED]"));
    assert_eq!(log.attributes.get("token").map(|s| s.as_str()), Some("[REDACTED]"));
    assert!(log.attributes.get("email").is_some_and(|e| e.contains("***")), "email should be partially redacted");
    assert_eq!(log.attributes.get("username").map(|s| s.as_str()), Some("john"));
}
