package greplog

import "time"

type Options struct {
	Service        string
	ServiceName    string
	SocketPath     string
	TCPHost        string
	TCPPort        int

	CaptureBodies  bool

	ReconnectDelay time.Duration

	PanicPolicy    PanicPolicy
}

type Config = Options
type Details = map[string]string

type PanicPolicy int

const (
	PanicPolicyReRaise PanicPolicy = iota
	PanicPolicyLog
)

