package greplog

import "time"

type Options struct {
	ServiceName string
	SocketPath  string
	TCPHost     string
	TCPPort     int

	CaptureBodies bool

	ReconnectDelay time.Duration

	PanicPolicy PanicPolicy
}

type PanicPolicy int

const (
	PanicPolicyReRaise PanicPolicy = iota
	PanicPolicyLog
)
