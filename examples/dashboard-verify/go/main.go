// Manual dashboard verification — Go traffic generator.
//
// Produces real HTTP traffic through the SDK's gin middleware, which
// auto-captures each request as a Span (typed status_code / start_time /
// end_time columns). This is the Go/Rust arm of the dual-source
// StatusCodesChart and AvgLatencyByServiceChart queries (spans UNION ALL
// logs.attributes), and the only way the spans arm ever gets data.
//
// In addition to spans, it also emits MANUAL greplog.* log events at every
// level and periodically re-runs a simulated panic through greplog.Go().
// Without these the `go-api` service contributes zero rows to the `logs`
// table, so the Errors page, error-type filter, severity distribution,
// noisy-services and error-rate-by-service charts would all be empty for
// this service. The panic capture (PanicPolicyLog) is what produces rows
// with exception_type = 'panic' + a real stack trace, feeding the Errors
// page error-type filter and the error drawer's Stack Trace section.
//
// ~10% of /orders requests return HTTP 500 with a short random sleep so the
// latency percentiles / avg-latency charts have real spread.
//
// Env:
//   GREPLOG_SERVICE  service name shown in the dashboard (default go-api)
//   GREPLOG_SOCKET   UDS path; falls back to TCP 127.0.0.1:4318 if absent
//   PORT             HTTP listen port (default 8080)
//
// Drive traffic with ../traffic.sh (the middleware only captures requests
// that actually happen — it generates no traffic on its own).
//
// Run from this dir: `go run .`

package main

import (
	"fmt"
	"log"
	"math/rand"
	"net/http"
	"os"
	"time"

	"github.com/gin-gonic/gin"
	greplog "github.com/greplog/greplog-go/src"
)

func env(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

func main() {
	greplog.Init(&greplog.Options{
		Service:      env("GREPLOG_SERVICE", "go-api"),
		SocketPath:   env("GREPLOG_SOCKET", ".greplog/greplog.sock"),
		CaptureBodies: true,
		PanicPolicy:  greplog.PanicPolicyLog,
	})
	defer greplog.Shutdown()

	// Manual greplog.* logs at every level (logger_name = 'greplog'). These
	// feed the log-volume, severity, error-rate and noisy-services charts.
	go func() {
		for {
			time.Sleep(800 * time.Millisecond)
			route := "/orders"
			if rand.Float64() < 0.5 {
				route = "/health"
			}
			greplog.Info("handled request", map[string]string{"route": route, "method": "GET", "http_status_code": "200"})
			if rand.Float64() < 0.2 {
				greplog.Warn("slow upstream", map[string]string{"route": route, "method": "GET", "latency_ms": "120"})
			}
			if rand.Float64() < 0.15 {
				greplog.Error("payment declined", map[string]string{
					"order_id":   "ord-" + randomIntString(),
					"amount":     "99.99",
					"route":      route,
					"error_code": "card_declined",
				})
			}
			greplog.Debug("cache miss", map[string]string{"route": route})
		}
	}()

	// Simulated panics through greplog.Go(). PanicPolicyLog keeps the process
	// alive while the SDK logs the panic with exception_type='panic' and a
	// real stack trace — the only Go path that populates those columns.
	go func() {
		for {
			time.Sleep(2500 * time.Millisecond)
			if rand.Float64() < 0.5 {
				greplog.Go(func() {
					panic("simulated panic in go-api")
				})
			}
		}
	}()

	r := gin.New()
	r.Use(gin.Logger(), gin.Recovery())
	r.Use(greplog.Middleware())

	r.GET("/orders", func(c *gin.Context) {
		if rand.Float64() < 0.1 {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "internal"})
			return
		}
		time.Sleep(time.Duration(rand.Intn(200)) * time.Millisecond)
		c.JSON(http.StatusOK, gin.H{"ok": true})
	})

	r.GET("/health", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"ok": true})
	})

	addr := "127.0.0.1:" + env("PORT", "8080")
	if err := r.Run(addr); err != nil {
		log.Fatalf("greplog-verify-go: %v", err)
	}
}

func randomIntString() string {
	return fmt.Sprintf("%d", rand.Intn(100000))
}
