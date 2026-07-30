package greplog

import (
	"encoding/binary"
	"io"
	"log"
	"net"

	core "github.com/greplog/greplog-go/core/v1"
	"google.golang.org/protobuf/proto"
)

// ResetTestState resets all internal state so tests start clean.
func ResetTestState() {
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

// TestServer wraps the internal test server for external tests.
type TestServer struct {
	inner *testServer
}

func (ts *TestServer) Addr() string                   { return ts.inner.addr() }
func (ts *TestServer) Close()                         { ts.inner.close() }
func (ts *TestServer) Frames() chan *core.IngestBatch { return ts.inner.frames }

// StartTestServer creates a TCP test server and returns a wrapper.
func StartTestServer() *TestServer {
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		log.Fatalf("failed to listen: %v", err)
	}
	ts := &testServer{
		ln:     ln,
		frames: make(chan *core.IngestBatch, 100),
	}
	go ts.accept()
	return &TestServer{inner: ts}
}

// PortFromAddr parses an IP:port string and returns the port number.
func PortFromAddr(addr string) int {
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

// ---------------------------------------------------------------------------
// Internal test helpers
// ---------------------------------------------------------------------------

type testServer struct {
	ln     net.Listener
	frames chan *core.IngestBatch
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
