// Manual dashboard verification — Go traffic generator.
//
// Produces real HTTP traffic through the SDK's gin middleware, which
// auto-captures each request as a Span (typed status_code / start_time /
// end_time columns). This is the Go/Rust arm of the dual-source
// StatusCodesChart and AvgLatencyByServiceChart queries (spans UNION ALL
// logs.attributes), and the only way the spans arm ever gets data.
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
		Service:    env("GREPLOG_SERVICE", "go-api"),
		SocketPath: env("GREPLOG_SOCKET", ".greplog/greplog.sock"),
	})
	defer greplog.Shutdown()

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
		panic(err)
	}
}
