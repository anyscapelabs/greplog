import greplog

# 1. Auto-capture only — captures uncaught exceptions and root log events automatically
greplog.init()

# 2. Service name override
greplog.init(service="api-backend")

# Fail-open guarantee: these calls are safe even if the agent isn't running
# 3. Manual log levels with structured details
greplog.error("Payment failed", details={"order_id": "ord-123", "amount": "99.99"})
greplog.warn("Retrying request", details={"attempt": "2"})
greplog.info("Server started", details={"port": "4000"})
greplog.debug("Cache miss", details={"key": "user:123"})
