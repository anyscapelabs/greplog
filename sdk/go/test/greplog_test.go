package greplog_test

import (
	"encoding/json"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"

	greplog "github.com/anyscapelabs/greplog/sdk/go/greplog"
)

// collectServer returns an httptest server that appends every received log record.
func collectServer(t *testing.T) (*httptest.Server, func() []greplog.LogEntry) {
	t.Helper()
	var (
		mu      sync.Mutex
		records []greplog.LogEntry
	)
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		defer mu.Unlock()
		var incoming []greplog.LogEntry
		if err := json.NewDecoder(r.Body).Decode(&incoming); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		records = append(records, incoming...)
		w.WriteHeader(http.StatusOK)
	}))
	get := func() []greplog.LogEntry {
		mu.Lock()
		defer mu.Unlock()
		out := make([]greplog.LogEntry, len(records))
		copy(out, records)
		return out
	}
	t.Cleanup(srv.Close)
	return srv, get
}

func waitFor(t *testing.T, cond func() bool) {
	t.Helper()
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		if cond() {
			return
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatal("condition not satisfied within deadline")
}

// mustHandler builds a handler from configuration known to be valid.
func mustHandler(t *testing.T, cfg greplog.Config) *greplog.Handler {
	t.Helper()
	h, err := greplog.NewHandler(cfg)
	if err != nil {
		t.Fatalf("NewHandler failed: %v", err)
	}
	return h
}

func TestDefaultConfigApplied(t *testing.T) {
	t.Setenv("GREPLOG_URL", "")
	t.Setenv("GREPLOG_SERVICE_NAME", "")
	client, err := greplog.NewClient(greplog.Config{Service: "svc"})
	if err != nil {
		t.Fatalf("NewClient failed: %v", err)
	}
	defer client.Close()

	if got := client.Endpoint(); got != "http://127.0.0.1:5050/api/log" {
		t.Fatalf("unexpected default endpoint %q", got)
	}
	if got := client.Service(); got != "svc" {
		t.Fatalf("service not preserved: %q", got)
	}
	if got := client.BatchSize(); got != 100 {
		t.Fatalf("unexpected default batch size %d", got)
	}
	if got := client.ChannelCapacity(); got != 10000 {
		t.Fatalf("unexpected default capacity %d", got)
	}
}

// TestRejectsInvalidServiceNames pins the client-side mirror of the server's
// ingest rule: the service name becomes a storage partition directory, so a
// traversal like ../evil must fail loudly at init instead of every record
// being rejected (and dropped) by the server later.
func TestRejectsInvalidServiceNames(t *testing.T) {
	t.Setenv("GREPLOG_SERVICE_NAME", "")

	if _, err := greplog.NewClient(greplog.Config{Service: "../evil"}); err == nil {
		t.Fatal("expected NewClient to reject a path-traversing service name")
	}
	cleanup, err := greplog.Init(greplog.Config{Service: "has space"})
	if err == nil {
		cleanup()
		t.Fatal("expected Init to report the invalid service name")
	}
	if cleanup != nil {
		t.Fatal("a failed Init must not return a cleanup function")
	}
	if !strings.Contains(err.Error(), "invalid service name") {
		t.Fatalf("error should name the problem, got %q", err)
	}

	func() {
		defer func() {
			if recover() == nil {
				t.Fatal("MustInit must panic on an invalid service name")
			}
		}()
		cleanup := greplog.MustInit(greplog.Config{Service: "../evil"})
		_ = cleanup // unreachable when MustInit panics as required
	}()
	ok := greplog.MustInit(greplog.Config{Service: "fine"})
	if ok == nil {
		t.Fatal("MustInit must return a cleanup function on success")
	}
	ok()

	for _, name := range []string{"auth-api", "payment_worker.2", "A-1_2.b", strings.Repeat("x", 64)} {
		if !greplog.IsValidServiceName(name) {
			t.Fatalf("ordinary service name %q must be accepted", name)
		}
	}
	for _, name := range []string{"", "../evil", "..\\windows", "a/b", "has space", "süß", strings.Repeat("x", 65)} {
		if greplog.IsValidServiceName(name) {
			t.Fatalf("service name %q must be rejected", name)
		}
	}
}

func TestFlushesOnBatchSize(t *testing.T) {
	srv, get := collectServer(t)
	h := mustHandler(t, greplog.Config{
		Service:       "billing",
		Endpoint:      srv.URL,
		BatchSize:     3,
		FlushInterval: time.Hour, // only the batch-size trigger fires
	})
	logger := slog.New(h)

	for i := 0; i < 5; i++ {
		logger.Info("processed", "i", i)
	}
	h.Close() // drains the remaining 2

	if got := len(get()); got != 5 {
		t.Fatalf("expected 5 records, got %d", got)
	}
}

func TestFlushesOnInterval(t *testing.T) {
	srv, get := collectServer(t)
	h := mustHandler(t, greplog.Config{
		Service:       "billing",
		Endpoint:      srv.URL,
		BatchSize:     1000,
		FlushInterval: 50 * time.Millisecond,
	})
	defer h.Close()

	slog.New(h).Warn("slow", "path", "/reports")
	waitFor(t, func() bool { return len(get()) == 1 })
	if got := get()[0].Message; got != "slow" {
		t.Fatalf("expected message slow, got %q", got)
	}
}

func TestWireSchema(t *testing.T) {
	srv, get := collectServer(t)
	h := mustHandler(t, greplog.Config{
		Service:       "billing",
		Endpoint:      srv.URL,
		BatchSize:     1,
		FlushInterval: time.Hour,
	})
	defer h.Close()

	slog.New(h).Error("payment declined", "order_id", 42)
	waitFor(t, func() bool { return len(get()) == 1 })

	record := get()[0]
	if record.TimestampUS <= 0 {
		t.Fatalf("expected non-zero timestamp_us, got %d", record.TimestampUS)
	}
	if record.Level != "ERROR" {
		t.Fatalf("unexpected level %q", record.Level)
	}
	if record.Service != "billing" {
		t.Fatalf("unexpected service %q", record.Service)
	}
	if record.RawBody == nil {
		t.Fatal("expected raw_body payload")
	}
	var details map[string]interface{}
	if err := json.Unmarshal([]byte(*record.RawBody), &details); err != nil {
		t.Fatalf("raw_body not JSON: %v", err)
	}
	if details["order_id"] != float64(42) {
		t.Fatalf("unexpected order_id %v", details["order_id"])
	}
}

func TestAttrsAndGroups(t *testing.T) {
	srv, get := collectServer(t)
	h := mustHandler(t, greplog.Config{
		Service:       "checkout",
		Endpoint:      srv.URL,
		BatchSize:     1,
		FlushInterval: time.Hour,
	})
	defer h.Close()

	logger := slog.New(h).With("env", "prod").WithGroup("request")
	logger.Info("payment ok", "id", "req_1")
	waitFor(t, func() bool { return len(get()) == 1 })

	entry := get()[0]
	if entry.RawBody == nil {
		t.Fatal("expected details payload")
	}
	var details map[string]interface{}
	if err := json.Unmarshal([]byte(*entry.RawBody), &details); err != nil {
		t.Fatalf("raw_body not JSON: %v", err)
	}
	if details["env"] != "prod" {
		t.Fatalf("With('env') attribute missing: %v", details)
	}
	if details["request.id"] != "req_1" {
		t.Fatalf("grouped attribute missing: %v", details)
	}
}

func TestGlobalDefaultLogger(t *testing.T) {
	srv, get := collectServer(t)
	cleanup, err := greplog.Init(greplog.Config{
		Service:       "svc",
		Endpoint:      srv.URL,
		BatchSize:     1,
		FlushInterval: time.Hour,
	})
	if err != nil {
		t.Fatalf("Init failed: %v", err)
	}
	defer cleanup()

	slog.Info("hello from slog", "k", "v")
	waitFor(t, func() bool { return len(get()) == 1 })

	entry := get()[0]
	if entry.Message != "hello from slog" {
		t.Fatalf("unexpected message %q", entry.Message)
	}
	if entry.Service != "svc" {
		t.Fatalf("unexpected service %q", entry.Service)
	}
	if !strings.Contains(entry.Level, "INFO") {
		t.Fatalf("unexpected level %q", entry.Level)
	}
}

// failThenSucceedServer returns a server that answers the first n requests
// with status and every request after that with 200 OK.
func failThenSucceedServer(t *testing.T, status int, failCount int) (*httptest.Server, func() int, func() []greplog.LogEntry) {
	t.Helper()
	var (
		mu      sync.Mutex
		seen    int
		records []greplog.LogEntry
	)
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		seen++
		fail := seen <= failCount
		mu.Unlock()

		if fail {
			w.WriteHeader(status)
			return
		}

		var incoming []greplog.LogEntry
		if err := json.NewDecoder(r.Body).Decode(&incoming); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		mu.Lock()
		records = append(records, incoming...)
		mu.Unlock()
		w.WriteHeader(http.StatusOK)
	}))
	requestCount := func() int {
		mu.Lock()
		defer mu.Unlock()
		return seen
	}
	get := func() []greplog.LogEntry {
		mu.Lock()
		defer mu.Unlock()
		out := make([]greplog.LogEntry, len(records))
		copy(out, records)
		return out
	}
	t.Cleanup(srv.Close)
	return srv, requestCount, get
}

// TestRetriesOn500 pins the fix for the bug where the SDK treated any HTTP
// response — including server errors — as a successful delivery. A batch
// that gets a 500 must be retried, not silently discarded.
func TestRetriesOn500(t *testing.T) {
	srv, _, get := failThenSucceedServer(t, http.StatusInternalServerError, 1)
	h := mustHandler(t, greplog.Config{
		Service:       "svc",
		Endpoint:      srv.URL,
		BatchSize:     1,
		FlushInterval: 30 * time.Millisecond,
	})
	defer h.Close()

	slog.New(h).Error("wal write failed upstream")
	waitFor(t, func() bool { return len(get()) == 1 })

	if got := h.Client().DroppedCount(); got != 0 {
		t.Fatalf("record delivered after retry must not be counted as dropped, got %d", got)
	}
}

// TestRetriesOn429 covers the rate-limited case: a 429 must be retried the
// same way a 500 is, not treated as a permanent rejection.
func TestRetriesOn429(t *testing.T) {
	srv, _, get := failThenSucceedServer(t, http.StatusTooManyRequests, 2)
	h := mustHandler(t, greplog.Config{
		Service:       "svc",
		Endpoint:      srv.URL,
		BatchSize:     1,
		FlushInterval: 20 * time.Millisecond,
	})
	defer h.Close()

	slog.New(h).Info("rate limited")
	waitFor(t, func() bool { return len(get()) == 1 })

	if got := h.Client().DroppedCount(); got != 0 {
		t.Fatalf("record delivered after retry must not be counted as dropped, got %d", got)
	}
}

// TestDropsOnPermanentRejection pins the other half of the fix: a 4xx that
// isn't a 429 means the server rejected this exact payload, so the SDK must
// drop it (and count it) rather than retry an identical request forever.
func TestDropsOnPermanentRejection(t *testing.T) {
	srv, requestCount, get := failThenSucceedServer(t, http.StatusUnprocessableEntity, 1000)
	h := mustHandler(t, greplog.Config{
		Service:       "svc",
		Endpoint:      srv.URL,
		BatchSize:     1,
		FlushInterval: 20 * time.Millisecond,
	})

	slog.New(h).Info("malformed somehow")
	waitFor(t, func() bool { return h.Client().DroppedCount() == 1 })
	h.Close()

	if got := len(get()); got != 0 {
		t.Fatalf("rejected record must never be recorded as delivered, got %d", got)
	}
	// A single flush attempt, not a hammering retry loop, must have happened
	// for the one rejected record.
	if got := requestCount(); got > 3 {
		t.Fatalf("expected at most a couple of attempts for a 4xx rejection, got %d", got)
	}
}

// TestRetryDoesNotHammerOnEveryEntry pins the backoff behavior: once a flush
// fails retryably, new entries must not each re-trigger an immediate resend
// against a server that is already failing — retries should only happen on
// the flush interval ticker.
func TestRetryDoesNotHammerOnEveryEntry(t *testing.T) {
	srv, requestCount, get := failThenSucceedServer(t, http.StatusServiceUnavailable, 3)
	h := mustHandler(t, greplog.Config{
		Service:       "svc",
		Endpoint:      srv.URL,
		BatchSize:     1, // every Info() call alone would exceed the batch size
		FlushInterval: 50 * time.Millisecond,
	})
	defer h.Close()

	logger := slog.New(h)
	// Fire many entries in a burst while the server is failing. If a failed
	// batch re-triggered on every new entry, this alone would produce dozens
	// of requests before the first retry-worthy tick even happens.
	for i := 0; i < 20; i++ {
		logger.Info("burst", "i", i)
	}

	waitFor(t, func() bool { return len(get()) > 0 })

	// Only ticker-paced retries should have fired: a handful of attempts,
	// not one per burst entry.
	if got := requestCount(); got > 6 {
		t.Fatalf("retries must be paced by the ticker, not by every incoming entry; got %d requests", got)
	}
}

// TestFullChannelDropsWithoutBlocking pins the "never stall the host app"
// contract: once the buffer is full (worker blocked on a hanging server),
// Handle returns immediately instead of blocking.
func TestFullChannelDropsWithoutBlocking(t *testing.T) {
	release := make(chan struct{})
	blocking := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		<-release
		w.WriteHeader(http.StatusOK)
	}))
	defer blocking.Close()

	h := mustHandler(t, greplog.Config{
		Service:         "svc",
		Endpoint:        blocking.URL,
		BatchSize:       1000,
		ChannelCapacity: 2,
		FlushInterval:   time.Hour,
	})
	defer h.Close()
	logger := slog.New(h)

	done := make(chan struct{})
	go func() {
		for i := 0; i < 200; i++ {
			logger.Info("spam", "i", i)
		}
		close(done)
	}()

	select {
	case <-done:
	case <-time.After(2 * time.Second):
		t.Fatal("Handle blocked: non-blocking channel contract violated")
	}

	close(release)
}
