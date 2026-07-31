/*
Package greplog provides a Go SDK for the Greplog observability agent.

Usage:

	import "github.com/greplog/greplog-go/src"

	greplog.Init(&greplog.Options{ServiceName: "my-service"})
	defer greplog.Shutdown()

	greplog.Info("server starting", map[string]string{"port": "8080"})
	greplog.Error("something failed", map[string]string{"err": err.Error()})

What Init() covers:
  - Transport setup (UDS/TCP dual-transport to the local agent)
  - Manual logging functions (Error, Warn, Info, Debug)
  - goroutine panic capture via greplog.Go()
  - Framework detection (reads go.mod for gin/echo/fiber)

What Init() does NOT cover (Go language constraints):
  - Automatic capture of panics from raw go func(){}() — you must use greplog.Go() instead
  - Automatic HTTP request/response capture — you must add middleware yourself:
        gin:    r.Use(greplog.Middleware())
        net/http: handler = greplog.WrapHandler(handler)
  - Global panic hooks — Go has no equivalent to Python's sys.excepthook
  - Automatic goroutine stack trace capture for goroutines not started via greplog.Go()

All public functions are fail-open: if the agent is unreachable, calls are silently
dropped — your application will never panic or crash due to a transport failure.
*/
package greplog

import (
	"encoding/json"
	"os"
	"sync"
	"sync/atomic"
	"time"

	core "github.com/greplog/greplog-go/core/v1"
)

var (
	gInit   atomic.Bool
	gMu     sync.Mutex
	gOpts   Options
	gLogs   []*core.LogEvent
	gSpans  []*core.Span
	gSeq    int64
	gTr     *transport
	gClosed bool
)

func Init(opts ...any) {
	if gInit.Load() {
		return
	}
	defer gInit.Store(true)

	var parsedOpts Options
	if len(opts) > 0 && opts[0] != nil {
		switch o := opts[0].(type) {
		case *Options:
			if o != nil {
				parsedOpts = *o
			}
		case Options:
			parsedOpts = o
		}
	}

	gMu.Lock()
	gOpts = parsedOpts
	if gOpts.ServiceName == "" && gOpts.Service != "" {
		gOpts.ServiceName = gOpts.Service
	}
	if gOpts.ServiceName == "" {
		gOpts.ServiceName = detectServiceName()
	}
	detectedFramework = detectFramework()
	gClosed = false
	gLogs = nil
	gSpans = nil
	gSeq = 0
	gMu.Unlock()

	tr := newTransport(gOpts.SocketPath, gOpts.TCPHost, gOpts.TCPPort, gOpts.ReconnectDelay)
	gMu.Lock()
	gTr = tr
	gMu.Unlock()

	if detectedFramework != FrameworkNone {
		writeConfigJSON()
	}
}

func writeConfigJSON() {
	type frameworkInfo struct {
		Detected string `json:"detected"`
	}
	type configData struct {
		Service   string          `json:"service"`
		Framework frameworkInfo   `json:"framework"`
		Timestamp time.Time       `json:"timestamp"`
	}

	names := map[Framework]string{
		FrameworkGin:   "gin",
		FrameworkEcho:  "echo",
		FrameworkFiber: "fiber",
	}
	fwName := names[detectedFramework]

	data := configData{
		Service:   gOpts.ServiceName,
		Framework: frameworkInfo{Detected: fwName},
		Timestamp: time.Now(),
	}
	raw, err := json.MarshalIndent(data, "", "  ")
	if err != nil {
		return
	}
	os.WriteFile("greplog.config.json", raw, 0644)
}

func Shutdown() {
	gMu.Lock()
	already := gClosed
	if !already {
		gClosed = true
	}
	tr := gTr
	gMu.Unlock()

	if already {
		return
	}
	flushLocked()
	if tr != nil {
		tr.shutdown()
	}
}

func Error(msg string, details ...map[string]string) {
	logEvent("error", msg, details...)
}

func Warn(msg string, details ...map[string]string) {
	logEvent("warn", msg, details...)
}

func Info(msg string, details ...map[string]string) {
	logEvent("info", msg, details...)
}

func Debug(msg string, details ...map[string]string) {
	logEvent("debug", msg, details...)
}

func logEvent(level, msg string, details ...map[string]string) {
	if !gInit.Load() {
		return
	}
	var attrs map[string]string
	if len(details) > 0 && details[0] != nil {
		attrs = redactAttributes(details[0])
	} else {
		attrs = make(map[string]string)
	}
	if attrs == nil {
		attrs = make(map[string]string)
	}

	evt := &core.LogEvent{
		ServiceName: serviceName(),
		Message:     msg,
		Level:       level,
		TimestampNs: time.Now().UnixNano(),
		LoggerName:  "greplog",
		Attributes:  attrs,
		EventId:     generateULID(),
	}

	gMu.Lock()
	gLogs = append(gLogs, evt)
	flushLocked()
	gMu.Unlock()
}

func flushLocked() {
	if gClosed || gTr == nil {
		return
	}
	if len(gLogs) == 0 && len(gSpans) == 0 {
		return
	}

	batchSize := 100
	var batchLogs []*core.LogEvent
	var batchSpans []*core.Span

	if len(gLogs) > 0 {
		if len(gLogs) > batchSize {
			batchLogs = gLogs[:batchSize]
			gLogs = gLogs[batchSize:]
		} else {
			batchLogs = gLogs
			gLogs = nil
		}
	}
	if len(gSpans) > 0 {
		if len(gSpans) > batchSize {
			batchSpans = gSpans[:batchSize]
			gSpans = gSpans[batchSize:]
		} else {
			batchSpans = gSpans
			gSpans = nil
		}
	}

	gSeq++
	frame, err := encodeIngestBatch(
		serviceName(),
		instanceID(),
		gSeq,
		batchLogs,
		batchSpans,
	)
	if err != nil {
		return
	}
	gTr.send(frame)
}

var (
	_serviceName string
	_instanceID  string
	_nameMu      sync.Mutex
)

func serviceName() string {
	_nameMu.Lock()
	defer _nameMu.Unlock()
	if _serviceName == "" {
		gMu.Lock()
		sn := gOpts.ServiceName
		gMu.Unlock()
		if sn == "" {
			sn = detectServiceName()
		}
		_serviceName = sn
	}
	return _serviceName
}

func instanceID() string {
	_nameMu.Lock()
	defer _nameMu.Unlock()
	if _instanceID == "" {
		_instanceID = detectInstanceID()
	}
	return _instanceID
}

func pushSpan(span *core.Span) {
	if !gInit.Load() {
		return
	}
	gMu.Lock()
	gSpans = append(gSpans, span)
	flushLocked()
	gMu.Unlock()
}
