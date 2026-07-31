fn main() {
    // 1. Auto-capture only — captures panics and tracing events automatically
    greplog::init();

    // 2. Service name override
    greplog::init_with_config(greplog::Config {
        service: Some("api-backend".into()),
        ..Default::default()
    });

    // Fail-open guarantee: these calls are safe even if the agent isn't running
    // 3. Manual log levels with structured details
    greplog::error!("payment failed", &[("order_id", "ord-123"), ("amount", "99.99")]);
    greplog::warn!("retrying request", &[("attempt", "2")]);
    greplog::info!("server started", &[("port", "4000")]);
    greplog::debug!("cache miss", &[("key", "user:123")]);
}
