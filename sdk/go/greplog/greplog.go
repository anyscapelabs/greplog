package greplog

import (
	"log/slog"
)

var defaultHandler *Handler

// Init sets up the Greplog SDK and configures it as the global default slog
// logger. All slog.Info, slog.Error, and structured slog calls in the process
// are then streamed to the Greplog ingest agent asynchronously.
//
// The returned cleanup function performs a graceful shutdown and must be
// called from main() (for example in a deferred call after wiring a signal
// handler for SIGINT/SIGTERM) to flush any pending logs.
func Init(cfg Config) func() {
	defaultHandler = NewHandler(cfg)
	logger := slog.New(defaultHandler)
	slog.SetDefault(logger)

	return func() {
		if defaultHandler != nil {
			defaultHandler.Close()
			defaultHandler = nil
		}
	}
}
