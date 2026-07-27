mod common;

use std::thread;
use std::time::Duration;

#[test]
fn test_panic_capture() {
    let (port, rx) = common::start_test_server();

    std::env::set_var("GREPLOG_TEST_PORT", port.to_string());
    greplog::init();

    thread::sleep(Duration::from_millis(200));

    let result = std::panic::catch_unwind(|| {
        panic!("test-panic-message");
    });
    assert!(result.is_err());

    thread::sleep(Duration::from_millis(500));
    let batches = common::wait_for_frames(&rx, 1);

    assert!(!batches.is_empty(), "expected at least one batch");
    let batch = &batches[0];
    assert!(!batch.logs.is_empty(), "expected at least one log event");
    let log = &batch.logs[0];
    assert_eq!(log.level, "error");
    assert_eq!(log.exception_type, "panic");
    assert!(log.message.contains("test-panic-message"));
    assert!(!log.event_id.is_empty());
}
