package greplog

import (
	"net/http"
	"time"

	"github.com/gin-gonic/gin"

	core "github.com/greplog/greplog-go/core/v1"
)

func Middleware() gin.HandlerFunc {
	return func(c *gin.Context) {
		start := time.Now()
		c.Next()
		duration := time.Since(start)

		route := c.FullPath()
		if route == "" {
			route = c.Request.URL.Path
		}

		span := &core.Span{
			ServiceName: serviceName(),
			Name:        c.Request.Method + " " + route,
			StartTimeNs: start.UnixNano(),
			EndTimeNs:   time.Now().UnixNano(),
			Route:       route,
			Method:      c.Request.Method,
			StatusCode:  int32(c.Writer.Status()),
			Kind:        core.SpanKind_SPAN_KIND_SERVER,
			EventId:     generateULID(),

			Attributes: captureHeaders(c.Request),
		}

		if duration > 0 {
			_ = duration
		}

		pushSpan(span)
	}
}

func WrapHandler(handler http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		sw := &statusWriter{ResponseWriter: w, status: http.StatusOK}
		handler.ServeHTTP(sw, r)
		duration := time.Since(start)

		span := &core.Span{
			ServiceName: serviceName(),
			Name:        r.Method + " " + r.URL.Path,
			StartTimeNs: start.UnixNano(),
			EndTimeNs:   time.Now().UnixNano(),
			Route:       r.URL.Path,
			Method:      r.Method,
			StatusCode:  int32(sw.status),
			Kind:        core.SpanKind_SPAN_KIND_SERVER,
			EventId:     generateULID(),

			Attributes: captureHeaders(r),
		}

		if duration > 0 {
			_ = duration
		}

		pushSpan(span)
	})
}

type statusWriter struct {
	http.ResponseWriter
	status int
}

func (sw *statusWriter) WriteHeader(code int) {
	sw.status = code
	sw.ResponseWriter.WriteHeader(code)
}

func captureHeaders(r *http.Request) map[string]string {
	if !gInit.Load() {
		return nil
	}
	gMu.Lock()
	captureBodies := gOpts.CaptureBodies
	gMu.Unlock()

	headers := make(map[string]string, len(r.Header))
	for k, vals := range r.Header {
		if len(vals) > 0 {
			headers[k] = vals[0]
		}
	}
	headers["host"] = r.Host

	if captureBodies && r.Body != nil {
		buf := make([]byte, 4096)
		n, _ := r.Body.Read(buf)
		if n > 0 {
			headers["body"] = string(buf[:n])
		}
	}

	return redactAttributes(headers)
}
