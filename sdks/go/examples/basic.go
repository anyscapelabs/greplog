package main

import (
	greplog "github.com/greplog/greplog-go/src"
)

func main() {
	// 1. Auto-capture only — captures panics (via greplog.Go) and HTTP metadata automatically
	greplog.Init()

	// 2. Service name override
	greplog.Init(greplog.Config{Service: "api-backend"})

	// Fail-open guarantee: these calls are safe even if the agent isn't running
	// 3. Manual log levels with structured details
	greplog.Error("payment failed", greplog.Details{"order_id": "ord-123", "amount": "99.99"})
	greplog.Warn("retrying request", greplog.Details{"attempt": "2"})
	greplog.Info("server started", greplog.Details{"port": "4000"})
	greplog.Debug("cache miss", greplog.Details{"key": "user:123"})
}
