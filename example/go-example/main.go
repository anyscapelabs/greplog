package main

import (
	"encoding/json"
	"log/slog"
	"net/http"
	"time"

	greplog "github.com/anyscapelabs/greplog/sdk/go/greplog"
)

func main() {
	// 1. Initialize Greplog as the global slog handler. Every standard
	//    slog.Info / slog.Error call below is automatically streamed to the
	//    Greplog backend (POST http://127.0.0.1:5050/api/log by default).
	// MustInit panics on invalid configuration (e.g. a service name the
	// server would reject) — startup should not continue half-configured.
	// Use greplog.Init instead when you want to handle the error yourself.
	cleanup := greplog.MustInit(greplog.Config{
		Service: "payment-gateway",
		Env:     "development",
		// Endpoint defaults to http://127.0.0.1:5050/api/log
	})
	// 2. Flush any pending asynchronous logs before the process exits.
	defer cleanup()

	// 3. Setup routes.
	http.HandleFunc("/api/charge", chargeHandler)

	// 4. Standard slog calls now route through Greplog.
	slog.Info("Server running on port 4001", slog.String("port", "4001"))
	slog.Info("Greplog auto-instrumentation active.")

	if err := http.ListenAndServe(":4001", nil); err != nil {
		slog.Error("server failed", slog.Any("err", err))
	}
}

// chargeHandler demonstrates structured contextual logging with the native
// log/slog library. No greplog-specific calls are required here.
func chargeHandler(w http.ResponseWriter, r *http.Request) {
	start := time.Now()

	// HTTP request logging via defer.
	defer func() {
		duration := time.Since(start).Milliseconds()
		slog.Info("HTTP request",
			slog.String("method", r.Method),
			slog.String("path", r.URL.Path),
			slog.Int64("duration_ms", duration),
			slog.String("ip", r.RemoteAddr),
		)
	}()

	var req struct {
		Amount int    `json:"amount"`
		UserID string `json:"user_id"`
	}

	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		slog.Warn("charge failed: invalid payload", slog.String("error", err.Error()))
		http.Error(w, "Invalid request", http.StatusBadRequest)
		return
	}

	if req.Amount <= 0 {
		slog.Warn("charge failed: invalid amount", slog.Int("amount", req.Amount))
		http.Error(w, "Amount must be greater than zero", http.StatusBadRequest)
		return
	}

	if req.Amount > 10000 {
		// Log rich structured data as contextual attributes.
		slog.Warn("high value transaction detected",
			slog.Int("amount", req.Amount),
			slog.String("user_id", req.UserID),
		)
	}

	slog.Info("payment processed successfully",
		slog.Int("amount", req.Amount),
		slog.String("user_id", req.UserID),
		slog.String("transaction_id", "tx_98765xyz"),
	)

	w.WriteHeader(http.StatusOK)
	w.Write([]byte(`{"status":"success", "transaction_id":"tx_98765xyz"}`))
}
