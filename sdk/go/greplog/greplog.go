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
//
// Init reports configuration problems — an invalid service name above all —
// instead of installing a logger whose every record the server would reject.
func Init(cfg Config) (func(), error) {
	handler, err := NewHandler(cfg)
	if err != nil {
		return nil, err
	}
	defaultHandler = handler
	logger := slog.New(defaultHandler)
	slog.SetDefault(logger)

	return func() {
		if defaultHandler != nil {
			defaultHandler.Close()
			defaultHandler = nil
		}
	}, nil
}
