package greplog

import (
	"fmt"
	"runtime"
	"time"

	core "github.com/greplog/greplog-go/core/v1"
)

func Go(fn func()) {
	go func() {
		defer recoverPanic()
		fn()
	}()
}

func recoverPanic() {
	r := recover()
	if r == nil {
		return
	}

	msg := fmt.Sprintf("%v", r)
	stack := make([]byte, 4096)
	n := runtime.Stack(stack, false)
	stackStr := string(stack[:n])

	evt := &core.LogEvent{
		ServiceName:      serviceName(),
		Message:          msg,
		Level:            "error",
		TimestampNs:      time.Now().UnixNano(),
		LoggerName:       "greplog",
		ExceptionType:    "panic",
		ExceptionMessage: msg,
		StackTrace:       []string{stackStr},
		EventId:          generateULID(),
	}

	if gInit.Load() {
		gMu.Lock()
		gLogs = append(gLogs, evt)
		flushLocked()
		gMu.Unlock()
	}

	gMu.Lock()
	policy := gOpts.PanicPolicy
	gMu.Unlock()

	if policy == PanicPolicyReRaise {
		panic(r)
	}
}
