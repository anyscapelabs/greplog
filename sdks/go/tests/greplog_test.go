package greplog_test

import (
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/gin-gonic/gin"

	greplog "github.com/greplog/greplog-go/src"
)

func setupWithServer(t *testing.T, opts *greplog.Options) *greplog.TestServer {
	ts := greplog.StartTestServer()
	t.Cleanup(ts.Close)

	if opts == nil {
		opts = &greplog.Options{}
	}
	opts.TCPPort = greplog.PortFromAddr(ts.Addr())
	opts.ReconnectDelay = 10 * time.Millisecond
	opts.ServiceName = "test-service"

	greplog.ResetTestState()
	greplog.Init(opts)
	t.Cleanup(greplog.Shutdown)

	return ts
}

// ──────────────────────────────────────────────
// Test 1: Fail-open (no agent)
// ──────────────────────────────────────────────

func Test_FailOpenNoAgent(t *testing.T) {
	greplog.ResetTestState()
	defer greplog.ResetTestState()

	greplog.Init(&greplog.Options{ReconnectDelay: 10 * time.Millisecond, PanicPolicy: greplog.PanicPolicyLog})

	greplog.Error("fail-open-error", map[string]string{"k": "v"})
	greplog.Warn("fail-open-warn")
	greplog.Info("fail-open-info")
	greplog.Debug("fail-open-debug")

	greplog.Go(func() {
		panic("fail-open-panic")
	})

	mw := greplog.Middleware()
	if mw == nil {
		t.Error("Middleware() should return non-nil even without agent")
	}

	wrapped := greplog.WrapHandler(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	if wrapped == nil {
		t.Error("WrapHandler should return non-nil even without agent")
	}
}

// ──────────────────────────────────────────────
// Test 2: Go() captures goroutine panic
// ──────────────────────────────────────────────

func Test_GoCapturesPanic(t *testing.T) {
	ts := setupWithServer(t, &greplog.Options{PanicPolicy: greplog.PanicPolicyLog})

	greplog.Go(func() {
		panic("test-panic-message")
	})

	select {
	case batch := <-ts.Frames():
		if len(batch.Logs) == 0 {
			t.Fatal("expected at least one log event")
		}
		evt := batch.Logs[0]
		if evt.Level != "error" {
			t.Errorf("expected level 'error', got %q", evt.Level)
		}
		if evt.Message != "test-panic-message" {
			t.Errorf("expected message 'test-panic-message', got %q", evt.Message)
		}
		if evt.ExceptionType != "panic" {
			t.Errorf("expected ExceptionType 'panic', got %q", evt.ExceptionType)
		}
		if len(evt.StackTrace) == 0 {
			t.Error("expected non-empty stack trace")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("timeout waiting for panic event")
	}
}

// ──────────────────────────────────────────────
// Test 3: Raw go func(){}() panics are NOT retroactively caught
// ──────────────────────────────────────────────

func Test_RawGoPanicNotCaptured(t *testing.T) {
	ts := setupWithServer(t, &greplog.Options{PanicPolicy: greplog.PanicPolicyLog})

	done := make(chan struct{})
	go func() {
		defer func() { recover(); close(done) }()
		panic("raw-panic-not-captured")
	}()
	<-done

	time.Sleep(200 * time.Millisecond)

	select {
	case <-ts.Frames():
		t.Error("raw goroutine panic was captured by greplog — this should NOT happen." +
			" Only greplog.Go() should capture goroutine panics.")
	default:
	}
}

// ──────────────────────────────────────────────
// Test 4: HTTP middleware captures data correctly
// ──────────────────────────────────────────────

func Test_GinMiddlewareCapturesHTTP(t *testing.T) {
	gin.SetMode(gin.ReleaseMode)
	ts := setupWithServer(t, nil)

	router := gin.New()
	router.Use(greplog.Middleware())
	router.GET("/test/:id", func(c *gin.Context) {
		c.String(200, "ok")
	})

	srv := httptest.NewServer(router)
	defer srv.Close()

	resp, err := http.Get(srv.URL + "/test/42")
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()

	select {
	case batch := <-ts.Frames():
		if len(batch.Spans) == 0 {
			t.Fatal("expected at least one span")
		}
		span := batch.Spans[0]
		if span.Name != "GET /test/:id" {
			t.Errorf("expected Name 'GET /test/:id', got %q", span.Name)
		}
		if span.Route != "/test/:id" {
			t.Errorf("expected Route '/test/:id', got %q", span.Route)
		}
		if span.Method != "GET" {
			t.Errorf("expected Method 'GET', got %q", span.Method)
		}
		if span.StatusCode != 200 {
			t.Errorf("expected StatusCode 200, got %d", span.StatusCode)
		}
		if span.StartTimeNs == 0 || span.EndTimeNs == 0 {
			t.Error("StartTimeNs and EndTimeNs must be non-zero")
		}
	case <-time.After(3 * time.Second):
		t.Fatal("timeout waiting for span from gin middleware")
	}
}

func Test_NetHTTPWrapHandler(t *testing.T) {
	ts := setupWithServer(t, nil)

	handler := greplog.WrapHandler(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusCreated)
		w.Write([]byte("created"))
	}))

	srv := httptest.NewServer(handler)
	defer srv.Close()

	resp, err := http.Get(srv.URL + "/resource")
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()

	select {
	case batch := <-ts.Frames():
		if len(batch.Spans) == 0 {
			t.Fatal("expected at least one span")
		}
		span := batch.Spans[0]
		if span.Method != "GET" {
			t.Errorf("expected Method 'GET', got %q", span.Method)
		}
		if span.StatusCode != 201 {
			t.Errorf("expected StatusCode 201, got %d", span.StatusCode)
		}
		if _, ok := span.Attributes["host"]; !ok {
			t.Error("expected 'host' in attributes")
		}
	case <-time.After(3 * time.Second):
		t.Fatal("timeout waiting for span from WrapHandler")
	}
}

// ──────────────────────────────────────────────
// Test 5: Manual API independent of Init()
// ──────────────────────────────────────────────

func Test_ManualAPIIndependentOfInit(t *testing.T) {
	greplog.ResetTestState()
	defer greplog.ResetTestState()

	greplog.Error("no-init-error", map[string]string{"k": "v"})
	greplog.Warn("no-init-warn")
	greplog.Info("no-init-info")
	greplog.Debug("no-init-debug")
}

// ──────────────────────────────────────────────
// Test 6: Redaction applies uniformly
// ──────────────────────────────────────────────

func Test_RedactionApplies(t *testing.T) {
	ts := setupWithServer(t, nil)

	greplog.Info("login", map[string]string{
		"password": "secret123",
		"email":    "user@example.com",
		"token":    "abc-def-ghi",
		"username": "john",
	})

	select {
	case batch := <-ts.Frames():
		if len(batch.Logs) == 0 {
			t.Fatal("expected at least one log event")
		}
		attrs := batch.Logs[0].Attributes
		if attrs["password"] != "[REDACTED]" {
			t.Errorf("expected password to be '[REDACTED]', got %q", attrs["password"])
		}
		if attrs["token"] != "[REDACTED]" {
			t.Errorf("expected token to be '[REDACTED]', got %q", attrs["token"])
		}
		if len(attrs["email"]) < 5 || attrs["email"] == "user@example.com" {
			t.Errorf("expected email to be partially redacted, got %q", attrs["email"])
		}
		if attrs["username"] != "john" {
			t.Errorf("expected username to be unchanged, got %q", attrs["username"])
		}
	case <-time.After(2 * time.Second):
		t.Fatal("timeout waiting for redaction test event")
	}
}

func Test_RedactionViaMiddleware(t *testing.T) {
	gin.SetMode(gin.ReleaseMode)
	ts := setupWithServer(t, nil)

	router := gin.New()
	router.Use(greplog.Middleware())
	router.GET("/login", func(c *gin.Context) {
		c.String(200, "ok")
	})

	srv := httptest.NewServer(router)
	defer srv.Close()

	req, _ := http.NewRequest("GET", srv.URL+"/login", nil)
	req.Header.Set("X-Secret-Token", "Bearer secret-token")
	req.Header.Set("Email", "admin@example.com")
	req.Header.Set("Username", "john")
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()

	select {
	case batch := <-ts.Frames():
		if len(batch.Spans) == 0 {
			t.Fatal("expected a span")
		}
		attrs := batch.Spans[0].Attributes
		if attrs["X-Secret-Token"] != "[REDACTED]" {
			t.Errorf("expected X-Secret-Token to be [REDACTED], got %q", attrs["X-Secret-Token"])
		}
		if len(attrs["Email"]) < 5 || attrs["Email"] == "admin@example.com" {
			t.Errorf("expected Email to be partially redacted, got %q", attrs["Email"])
		}
		if attrs["Username"] != "john" {
			t.Errorf("expected Username to be unchanged, got %q", attrs["Username"])
		}
	case <-time.After(2 * time.Second):
		t.Fatal("timeout waiting for middleware redaction test")
	}
}

// ──────────────────────────────────────────────
// Test 7: Init() idempotency
// ──────────────────────────────────────────────

func Test_InitIdempotent(t *testing.T) {
	ts := greplog.StartTestServer()
	t.Cleanup(ts.Close)

	greplog.ResetTestState()

	port := greplog.PortFromAddr(ts.Addr())

	greplog.Init(&greplog.Options{
		TCPPort:        port,
		ServiceName:    "first-init",
		ReconnectDelay: 10 * time.Millisecond,
	})
	greplog.Init(&greplog.Options{
		TCPPort:        port,
		ServiceName:    "second-init",
		ReconnectDelay: 10 * time.Millisecond,
	})

	greplog.Info("idempotent-test")

	select {
	case batch := <-ts.Frames():
		if batch.ServiceName != "first-init" {
			t.Errorf("expected ServiceName 'first-init' (from first Init()), got %q", batch.ServiceName)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("timeout waiting for idempotent test event")
	}

	greplog.Shutdown()
}
