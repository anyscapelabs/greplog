// Package greplog provides an ultra-high-throughput structured logging client
// that streams records asynchronously to the Greplog ingest agent.
package greplog

import (
	"bytes"
	"encoding/json"
	"net/http"
	"os"
	"sync"
	"sync/atomic"
	"time"
)

// Config defines initialization parameters for the Go SDK.
type Config struct {
	Service   string `json:"service"`
	Env       string `json:"env,omitempty"`
	Endpoint  string `json:"endpoint,omitempty"`
	BatchSize int    `json:"batch_size,omitempty"`
	// FlushInterval is the maximum time a batch may age before a flush.
	FlushInterval   time.Duration `json:"flush_interval,omitempty"`
	ChannelCapacity int           `json:"channel_capacity,omitempty"`
	// MaxQueueSize caps the records held for retry after a failed flush;
	// the oldest are dropped beyond this so a downed server can never grow
	// process memory without bound.
	MaxQueueSize int `json:"max_queue_size,omitempty"`
}

// LogEntry is the JSON wire record accepted by the Greplog ingest agent
// (POST /api/log, port 5050). Field names match the Rust LogRecord schema.
type LogEntry struct {
	TimestampUS int64   `json:"timestamp_us"`
	TraceID     *string `json:"trace_id,omitempty"`
	Level       string  `json:"level"`
	Service     string  `json:"service"`
	Message     string  `json:"message"`
	// RawBody is a stringified JSON payload with the structured attributes.
	RawBody *string `json:"raw_body,omitempty"`
}

// Client is the non-blocking batching engine. Log writes go through a buffered
// channel and return immediately; a background worker builds batches and
// streams them over HTTP.
type Client struct {
	config     Config
	logChan    chan LogEntry
	httpClient *http.Client
	wg         sync.WaitGroup
	done       chan struct{}
	closeOnce  sync.Once
	dropped    atomic.Int64
}

func envDefault(value, key, fallback string) string {
	if value != "" {
		return value
	}
	if env := os.Getenv(key); env != "" {
		return env
	}
	return fallback
}

// NewClient builds a batching client with defaulted configuration. The raw
// low-level API; most users configure Greplog through NewHandler or Init
// instead.
func NewClient(cfg Config) *Client {
	// Apply defaults (with env-var fallbacks for zero-config deploys).
	cfg.Endpoint = envDefault(cfg.Endpoint, "GREPLOG_URL", "http://127.0.0.1:5050/api/log")
	cfg.Service = envDefault(cfg.Service, "GREPLOG_SERVICE_NAME", "go-app")
	cfg.Env = envDefault(cfg.Env, "GREPLOG_ENV", "development")
	if cfg.BatchSize <= 0 {
		cfg.BatchSize = 100
	}
	if cfg.FlushInterval <= 0 {
		cfg.FlushInterval = 500 * time.Millisecond
	}
	if cfg.ChannelCapacity <= 0 {
		cfg.ChannelCapacity = 10000
	}
	if cfg.MaxQueueSize <= 0 {
		cfg.MaxQueueSize = 10000
	}

	c := &Client{
		config:  cfg,
		logChan: make(chan LogEntry, cfg.ChannelCapacity),
		done:    make(chan struct{}),
		httpClient: &http.Client{
			Timeout: 5 * time.Second,
		},
	}

	c.wg.Add(1)
	go c.workerLoop()

	return c
}

// Service returns the effectively configured service name.
func (c *Client) Service() string { return c.config.Service }

// Endpoint returns the effectively configured ingest URL.
func (c *Client) Endpoint() string { return c.config.Endpoint }

// BatchSize returns the effectively configured batch size.
func (c *Client) BatchSize() int { return c.config.BatchSize }

// FlushInterval returns the effectively configured flush interval.
func (c *Client) FlushInterval() time.Duration { return c.config.FlushInterval }

// ChannelCapacity returns the effectively configured channel capacity.
func (c *Client) ChannelCapacity() int { return c.config.ChannelCapacity }

// DroppedCount returns the number of records dropped because the retry
// backlog exceeded MaxQueueSize, or because the server permanently rejected
// them (a non-retryable 4xx). It never counts records still awaiting retry.
func (c *Client) DroppedCount() int64 { return c.dropped.Load() }

// isRetryableStatus reports whether a non-2xx HTTP status is worth retrying:
// 5xx (server-side failure) and 429 (rate limited). Any other 4xx means the
// server rejected this exact request body, and resending identical bytes
// would only fail the same way forever.
func isRetryableStatus(status int) bool {
	return status == http.StatusTooManyRequests || status >= 500
}

func (c *Client) workerLoop() {
	defer c.wg.Done()

	ticker := time.NewTicker(c.config.FlushInterval)
	defer ticker.Stop()

	// pending holds records not yet durably accepted by the server. A
	// successful (or permanently rejected) flush empties it; a retryable
	// failure (network error, 5xx, 429) leaves it in place so the next
	// trigger resends it, merged with whatever arrived in the meantime.
	var pending []LogEntry
	// retrying is set once a flush fails retryably. While true, new entries
	// still accumulate into pending, but only the ticker re-attempts the
	// send — otherwise every single incoming entry would re-trigger an
	// immediate retry against a server that is already failing, turning an
	// outage into a hammering loop instead of a backoff.
	retrying := false

	attempt := func() {
		if len(pending) == 0 {
			return
		}
		if c.flush(pending) {
			pending = pending[:0]
			retrying = false
		} else {
			retrying = true
		}
		pending = c.capPending(pending)
	}

	for {
		select {
		case entry, ok := <-c.logChan:
			if !ok {
				// Channel closed: make one last attempt and exit.
				attempt()
				return
			}
			pending = append(pending, entry)
			if !retrying && len(pending) >= c.config.BatchSize {
				attempt()
			}

		case <-ticker.C:
			attempt()

		case <-c.done:
			// Drain the channel, flushing full batches as we go.
			for entry := range c.logChan {
				pending = append(pending, entry)
				if !retrying && len(pending) >= c.config.BatchSize {
					attempt()
				}
			}
			attempt()
			return
		}
	}
}

// capPending bounds the retry backlog at MaxQueueSize, dropping the oldest
// records first so a sustained outage can never grow process memory without
// bound.
func (c *Client) capPending(pending []LogEntry) []LogEntry {
	if len(pending) <= c.config.MaxQueueSize {
		return pending
	}
	overflow := len(pending) - c.config.MaxQueueSize
	c.dropped.Add(int64(overflow))
	return pending[overflow:]
}

// flush POSTs batch to the ingest endpoint and reports whether it is safe to
// discard: true means the server durably accepted it (2xx) or permanently
// rejected it (a non-retryable 4xx, counted as dropped); false means the
// caller should keep batch and retry — the record was never accepted.
func (c *Client) flush(batch []LogEntry) bool {
	data, err := json.Marshal(batch)
	if err != nil {
		// Will never marshal successfully no matter how many times we try.
		c.dropped.Add(int64(len(batch)))
		return true
	}

	req, err := http.NewRequest(http.MethodPost, c.config.Endpoint, bytes.NewReader(data))
	if err != nil {
		c.dropped.Add(int64(len(batch)))
		return true
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		// Network-level failure (server down, DNS, timeout): transient, retry.
		return false
	}
	defer func() { _ = resp.Body.Close() }()

	switch {
	case resp.StatusCode >= 200 && resp.StatusCode < 300:
		return true
	case isRetryableStatus(resp.StatusCode):
		return false
	default:
		// A non-retryable 4xx: the server rejected this exact payload, so
		// retrying identical bytes would only fail the same way forever.
		c.dropped.Add(int64(len(batch)))
		return true
	}
}

// Close gracefully stops the worker and flushes all remaining logs.
func (c *Client) Close() {
	c.closeOnce.Do(func() {
		close(c.done)
		close(c.logChan)
		c.wg.Wait()
	})
}
