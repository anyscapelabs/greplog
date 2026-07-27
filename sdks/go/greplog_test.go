package greplog

import (
	"encoding/binary"
	"io"
	"net"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/gin-gonic/gin"
	"google.golang.org/protobuf/proto"

	core "github.com/greplog/greplog-go/core/v1"
)

type testServer struct {
	ln     net.Listener
	frames chan *core.IngestBatch
	t      *testing.T
}

func startTestServer(t *testing.T) *testServer {
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("failed to listen: %v", err)
	}
	ts := &testServer{
		ln:     ln,
		frames: make(chan *core.IngestBatch, 100),
		t:      t,
	}
	go ts.accept()
	return ts
}

func (ts *testServer) accept() {
	conn, err := ts.ln.Accept()
	if err != nil {
		return
	}
	go ts.readFrames(conn)
}

func (ts *testServer) readFrames(conn net.Conn) {
	defer conn.Close()
	for {
		header := make([]byte, 4)
		if _, err := io.ReadFull(conn, header); err != nil {
			return
		}
		size := binary.LittleEndian.Uint32(header)
		data := make([]byte, size)
		if _, err := io.ReadFull(conn, data); err != nil {
			return
		}
		batch := &core.IngestBatch{}
		if err := proto.Unmarshal(data, batch); err != nil {
			continue
		}
		select {
		case ts.frames <- batch:
		default:
		}
	}
}

func (ts *testServer) addr() string {
	return ts.ln.Addr().String()
}

func (ts *testServer) close() {
	ts.ln.Close()
}

func portFromAddr(addr string) int {
	_, portStr, err := net.SplitHostPort(addr)
	if err != nil {
		return 0
	}
	var p int
	for _, c := range portStr {
		p = p*10 + int(c-'0')
	}
	return p
}

func setupWithServer(t *testing.T, opts *Options) *testServer {
	ts := startTestServer(t)
	t.Cleanup(ts.close)

	if opts == nil {
		opts = &Options{}
	}
	opts.TCPPort = portFromAddr(ts.addr())
	opts.ReconnectDelay = 10 * time.Millisecond
	opts.ServiceName = "test-service"

	Shutdown()
	warningPrinted = false
	gInit.Store(false)
	Init(opts)
	t.Cleanup(Shutdown)

	return ts
}

func resetState() {
	Shutdown()
	warningPrinted = false
	gInit.Store(false)
	gMu.Lock()
	gLogs = nil
	gSpans = nil
	gSeq = 0
	gTr = nil
	gClosed = false
	_serviceName = ""
	_instanceID = ""
	gMu.Unlock()
}

// ──────────────────────────────────────────────
// Test 1: Fail-open (no agent)
// ──────────────────────────────────────────────

func Test_FailOpenNoAgent(t *testing.T) {
	resetState()
	defer resetState()

	Init(&Options{ReconnectDelay: 10 * time.Millisecond, PanicPolicy: PanicPolicyLog})

	Error("fail-open-error", map[string]string{"k": "v"})
	Warn("fail-open-warn")
	Info("fail-open-info")
	Debug("fail-open-debug")

	Go(func() {
		panic("fail-open-panic")
	})

	mw := Middleware()
	if mw == nil {
		t.Error("Middleware() should return non-nil even without agent")
	}

	wrapped := WrapHandler(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	if wrapped == nil {
		t.Error("WrapHandler should return non-nil even without agent")
	}

	// If we reach here without panic, fail-open is confirmed
}

// ──────────────────────────────────────────────
// Test 2: Go() captures goroutine panic
// ──────────────────────────────────────────────

func Test_GoCapturesPanic(t *testing.T) {
	ts := setupWithServer(t, &Options{PanicPolicy: PanicPolicyLog})

	Go(func() {
		panic("test-panic-message")
	})

	select {
	case batch := <-ts.frames:
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
	ts := setupWithServer(t, &Options{PanicPolicy: PanicPolicyLog})

	done := make(chan struct{})
	go func() {
		defer func() { recover(); close(done) }()
		panic("raw-panic-not-captured")
	}()
	<-done

	time.Sleep(200 * time.Millisecond)

	// Our SDK must NOT have captured this panic
	select {
	case <-ts.frames:
		t.Error("raw goroutine panic was captured by greplog — this should NOT happen." +
			" Only greplog.Go() should capture goroutine panics.")
	default:
		// Expected: no events captured
	}
}

// ──────────────────────────────────────────────
// Test 4: HTTP middleware captures data correctly
// ──────────────────────────────────────────────

func Test_GinMiddlewareCapturesHTTP(t *testing.T) {
	gin.SetMode(gin.ReleaseMode)
	ts := setupWithServer(t, nil)

	router := gin.New()
	router.Use(Middleware())
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
	case batch := <-ts.frames:
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

	handler := WrapHandler(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
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
	case batch := <-ts.frames:
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
	resetState()
	defer resetState()

	Error("no-init-error", map[string]string{"k": "v"})
	Warn("no-init-warn")
	Info("no-init-info")
	Debug("no-init-debug")
}

// ──────────────────────────────────────────────
// Test 6: Redaction applies uniformly
// ──────────────────────────────────────────────

func Test_RedactionApplies(t *testing.T) {
	ts := setupWithServer(t, nil)

	Info("login", map[string]string{
		"password": "secret123",
		"email":    "user@example.com",
		"token":    "abc-def-ghi",
		"username": "john",
	})

	select {
	case batch := <-ts.frames:
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
	router.Use(Middleware())
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
	case batch := <-ts.frames:
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
	ts := startTestServer(t)
	t.Cleanup(ts.close)

	Shutdown()
	warningPrinted = false
	gInit.Store(false)
	gMu.Lock()
	gLogs = nil
	gSpans = nil
	gSeq = 0
	gTr = nil
	gClosed = false
	_serviceName = ""
	_instanceID = ""
	gMu.Unlock()

	port := portFromAddr(ts.addr())

	Init(&Options{
		TCPPort:        port,
		ServiceName:    "first-init",
		ReconnectDelay: 10 * time.Millisecond,
	})
	Init(&Options{
		TCPPort:        port,
		ServiceName:    "second-init",
		ReconnectDelay: 10 * time.Millisecond,
	})

	Info("idempotent-test")

	select {
	case batch := <-ts.frames:
		if batch.ServiceName != "first-init" {
			t.Errorf("expected ServiceName 'first-init' (from first Init()), got %q", batch.ServiceName)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("timeout waiting for idempotent test event")
	}

	Shutdown()
}
