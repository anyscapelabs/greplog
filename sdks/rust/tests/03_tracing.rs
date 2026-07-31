mod common;

use std::thread;
use std::time::Duration;

#[test]
fn test_tracing_layer_capture() {
    let (port, rx) = common::start_test_server();

    std::env::set_var("GREPLOG_TEST_PORT", port.to_string());
    greplog::init();

    thread::sleep(Duration::from_millis(200));

    tracing::info!("tracing-test-message");

    thread::sleep(Duration::from_millis(500));
    let batches = common::wait_for_frames(&rx, 1);

    assert!(!batches.is_empty(), "expected at least one batch");
    let batch = &batches[0];
    let has_tracing_event = batch
        .logs
        .iter()
        .any(|l| l.message.contains("tracing-test-message"));
    assert!(has_tracing_event, "expected tracing event to be captured");
}
