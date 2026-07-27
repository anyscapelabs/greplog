mod common;

use std::thread;
use std::time::Duration;

#[test]
fn test_init_idempotent() {
    let (port1, rx1) = common::start_test_server();

    std::env::set_var("GREPLOG_TEST_PORT", port1.to_string());
    greplog::init();

    // Second init with a different port should be ignored
    std::env::set_var("GREPLOG_TEST_PORT", "9999");
    greplog::init();

    thread::sleep(Duration::from_millis(200));
    greplog::info!("idempotent-test");
    thread::sleep(Duration::from_millis(500));

    let batches = common::wait_for_frames(&rx1, 1);
    assert!(!batches.is_empty(), "expected event on first port");
}
