package greplog

import (
	"encoding/binary"
	"log"
	"net"
	"strconv"
	"sync"
	"time"
)

const (
	defaultSocketPath     = ".greplog/greplog.sock"
	defaultTCPHost        = "127.0.0.1"
	defaultTCPPort        = 4318
	maxPendingFrames      = 1000
	defaultReconnectDelay = 5 * time.Second
)

type transport struct {
	connMu    sync.Mutex
	conn      net.Conn
	sockPath  string
	tcpHost   string
	tcpPort   int
	closed    bool
	writeCh   chan []byte
	pending   [][]byte
	pendingMu sync.Mutex
}

func newTransport(sockPath, tcpHost string, tcpPort int, reconnectDelay time.Duration) *transport {
	if sockPath == "" {
		sockPath = defaultSocketPath
	}
	if tcpHost == "" {
		tcpHost = defaultTCPHost
	}
	if tcpPort == 0 {
		tcpPort = defaultTCPPort
	}
	if reconnectDelay <= 0 {
		reconnectDelay = defaultReconnectDelay
	}
	t := &transport{
		sockPath: sockPath,
		tcpHost:  tcpHost,
		tcpPort:  tcpPort,
		writeCh:  make(chan []byte, 256),
	}
	go t.run(reconnectDelay)
	return t
}

func (t *transport) send(data []byte) {
	if t == nil {
		return
	}
	select {
	case t.writeCh <- data:
	default:
	}
}

func (t *transport) shutdown() {
	t.connMu.Lock()
	t.closed = true
	if t.conn != nil {
		t.conn.Close()
	}
	t.connMu.Unlock()
}

func (t *transport) trySendPending() {
	if t.closed {
		return
	}

	t.pendingMu.Lock()
	if len(t.pending) == 0 {
		t.pendingMu.Unlock()
		return
	}
	frames := t.pending
	t.pending = nil
	t.pendingMu.Unlock()

	t.connMu.Lock()
	conn := t.tryConnect()
	t.connMu.Unlock()

	if conn == nil {
		t.pendingMu.Lock()
		t.pending = append(t.pending, frames...)
		if len(t.pending) > maxPendingFrames {
			t.pending = t.pending[len(t.pending)-maxPendingFrames:]
		}
		t.pendingMu.Unlock()
		printWarning()
		return
	}

	for i, frame := range frames {
		if err := writeFrame(conn, frame); err != nil {
			t.connMu.Lock()
			if t.conn == conn {
				t.conn.Close()
				t.conn = nil
			}
			t.connMu.Unlock()
			t.pendingMu.Lock()
			t.pending = append(t.pending, frames[i:]...)
			if len(t.pending) > maxPendingFrames {
				t.pending = t.pending[len(t.pending)-maxPendingFrames:]
			}
			t.pendingMu.Unlock()
			printWarning()
			return
		}
	}
}

func (t *transport) tryConnect() net.Conn {
	if t.closed {
		return nil
	}
	if t.conn != nil {
		_, err := t.conn.Write(nil)
		if err == nil {
			return t.conn
		}
		t.conn.Close()
		t.conn = nil
	}
	conn := t.dial()
	t.conn = conn
	if conn != nil {
		t.flushPending(conn)
	}
	return conn
}

func (t *transport) dial() net.Conn {
	conn := t.dialUDS()
	if conn != nil {
		return conn
	}
	return t.dialTCP()
}

func (t *transport) dialUDS() net.Conn {
	conn, err := net.DialTimeout("unix", t.sockPath, 5*time.Second)
	if err != nil {
		return nil
	}
	return conn
}

func (t *transport) dialTCP() net.Conn {
	addr := net.JoinHostPort(t.tcpHost, strconv.Itoa(t.tcpPort))
	conn, err := net.DialTimeout("tcp", addr, 5*time.Second)
	if err != nil {
		return nil
	}
	return conn
}

func (t *transport) flushPending(conn net.Conn) {
	t.pendingMu.Lock()
	pending := t.pending
	t.pending = nil
	t.pendingMu.Unlock()

	for _, frame := range pending {
		if err := writeFrame(conn, frame); err != nil {
			t.pendingMu.Lock()
			t.pending = append(t.pending, frame)
			t.pendingMu.Unlock()
			return
		}
	}
}

func (t *transport) pend(frame []byte) {
	t.pendingMu.Lock()
	t.pending = append(t.pending, frame)
	if len(t.pending) > maxPendingFrames {
		t.pending = t.pending[1:]
	}
	t.pendingMu.Unlock()
}

func (t *transport) run(reconnectDelay time.Duration) {
	ticker := time.NewTicker(reconnectDelay)
	defer ticker.Stop()

	for {
		if t.closed {
			return
		}

		select {
		case frame, ok := <-t.writeCh:
			if !ok {
				return
			}
			t.pend(frame)
		case <-ticker.C:
		}

		t.trySendPending()
	}
}

func writeFrame(conn net.Conn, data []byte) error {
	header := make([]byte, 4)
	binary.LittleEndian.PutUint32(header, uint32(len(data)))
	if _, err := conn.Write(header); err != nil {
		return err
	}
	if _, err := conn.Write(data); err != nil {
		return err
	}
	return nil
}

var warningPrinted bool
var warningMu sync.Mutex

func printWarning() {
	warningMu.Lock()
	defer warningMu.Unlock()
	if warningPrinted {
		return
	}
	warningPrinted = true
	log.Println("[Greplog] Agent not found. Run 'greplog dev' to capture logs.")
}
