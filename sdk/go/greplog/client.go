// Package greplog provides an ultra-high-throughput structured logging client
// that streams records asynchronously to the Greplog ingest agent.
package greplog

import (
	"bytes"
	"encoding/json"
	"net/http"
	"os"
	"sync"
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

func (c *Client) workerLoop() {
	defer c.wg.Done()

	ticker := time.NewTicker(c.config.FlushInterval)
	defer ticker.Stop()

	var batch []LogEntry

	reset := func() {
		batch = make([]LogEntry, 0, c.config.BatchSize)
	}
	reset()

	for {
		select {
		case entry, ok := <-c.logChan:
			if !ok {
				// Channel closed: flush anything remaining and exit.
				if len(batch) > 0 {
					c.flush(batch)
				}
				return
			}
			batch = append(batch, entry)
			if len(batch) >= c.config.BatchSize {
				c.flush(batch)
				reset()
			}

		case <-ticker.C:
			if len(batch) > 0 {
				c.flush(batch)
				reset()
			}

		case <-c.done:
			// Drain the channel, flushing full batches as we go.
			for entry := range c.logChan {
				batch = append(batch, entry)
				if len(batch) >= c.config.BatchSize {
					c.flush(batch)
					reset()
				}
			}
			if len(batch) > 0 {
				c.flush(batch)
			}
			return
		}
	}
}

func (c *Client) flush(batch []LogEntry) {
	data, err := json.Marshal(batch)
	if err != nil {
		return
	}

	req, err := http.NewRequest(http.MethodPost, c.config.Endpoint, bytes.NewReader(data))
	if err != nil {
		return
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.httpClient.Do(req)
	if err == nil {
		_ = resp.Body.Close()
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
