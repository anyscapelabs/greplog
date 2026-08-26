package greplog

import (
	"context"
	"encoding/json"
	"log/slog"
	"strings"
	"time"
)

// Handler implements the standard library log/slog.Handler interface. Register
// it via slog.SetDefault to redirect every slog call in the application to
// Greplog automatically.
type Handler struct {
	client *Client
	attrs  []slog.Attr
	groups []string
}

// NewHandler builds a client and returns a slog handler bound to it. It
// fails before any log call when the configuration would be rejected by the
// server (e.g. an invalid service name).
func NewHandler(cfg Config) (*Handler, error) {
	client, err := NewClient(cfg)
	if err != nil {
		return nil, err
	}
	return &Handler{client: client}, nil
}

// Client returns the underlying batching client, for introspection (e.g.
// DroppedCount) or advanced use such as a manual Flush.
func (h *Handler) Client() *Client { return h.client }

// Enabled always reports true: every level is forwarded.
func (h *Handler) Enabled(_ context.Context, _ slog.Level) bool {
	return true
}

// Handle converts a slog record into a Greplog LogEntry and enqueues it
// through the non-blocking channel.
func (h *Handler) Handle(_ context.Context, r slog.Record) error {
	details := make(map[string]interface{})

	// Extract the structured record attributes (the details JSON payload).
	r.Attrs(func(a slog.Attr) bool {
		if len(h.groups) > 0 {
			a.Key = h.prefixGroup(a.Key)
		}
		details[a.Key] = a.Value.Any()
		return true
	})

	// Merge the attributes added via WithAttrs.
	for _, a := range h.attrs {
		details[a.Key] = a.Value.Any()
	}

	entry := newEntry(
		r.Level.String(),
		h.client.config.Service,
		r.Message,
		details,
	)

	// Non-blocking write: drop the entry when the channel is full so the host
	// application is never stalled by backpressure.
	select {
	case h.client.logChan <- entry:
	default:
	}

	return nil
}

// WithAttrs returns a handler with additional context, merged on every record.
func (h *Handler) WithAttrs(attrs []slog.Attr) slog.Handler {
	newAttrs := make([]slog.Attr, 0, len(h.attrs)+len(attrs))
	newAttrs = append(newAttrs, h.attrs...)
	newAttrs = append(newAttrs, attrs...)

	return &Handler{
		client: h.client,
		attrs:  newAttrs,
		groups: h.groups,
	}
}

// WithGroup prefixes grouped attribute keys with `name.group.` semantics.
func (h *Handler) WithGroup(name string) slog.Handler {
	newGroups := make([]string, 0, len(h.groups)+1)
	newGroups = append(newGroups, h.groups...)
	newGroups = append(newGroups, name)

	return &Handler{
		client: h.client,
		attrs:  h.attrs,
		groups: newGroups,
	}
}

// Close stops the batching worker and flushes pending logs.
func (h *Handler) Close() {
	h.client.Close()
}

// prefixGroup renders the slog group chain as `group1.group2.key`.
func (h *Handler) prefixGroup(key string) string {
	return strings.Join(append(append([]string{}, h.groups...), key), ".")
}

// newEntry builds a wire record with the ingest schema.
func newEntry(level, service, message string, details map[string]interface{}) LogEntry {
	entry := LogEntry{
		TimestampUS: time.Now().UnixMicro(),
		Level:       level,
		Service:     service,
		Message:     message,
	}
	if len(details) > 0 {
		raw, err := json.Marshal(details)
		if err == nil {
			body := string(raw)
			entry.RawBody = &body
		}
	}
	return entry
}
