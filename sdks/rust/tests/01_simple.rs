// Tests that do NOT need init() — share a process safely.

#[test]
fn test_fail_open_no_agent() {
    // Calling macros without init() should not panic
    greplog::info!("fail-open-info");
    greplog::error!("fail-open-error", &[("k", "v")]);
    greplog::warn!("fail-open-warn");
    greplog::debug!("fail-open-debug");
}

#[test]
fn test_manual_api_independent_of_init() {
    // manual_log must not panic when init() was never called
    greplog::info!("no-init-info");
    greplog::error!("no-init-error", &[("key", "val")]);
}

#[test]
fn test_reuse_confirmed() {
    use greplog_core::gen::LogEvent;
    let _ = LogEvent::default();
}
