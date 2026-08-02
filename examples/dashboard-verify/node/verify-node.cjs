#!/usr/bin/env node
// Manual dashboard verification — Node.js traffic generator.
//
// Emits BOTH kinds of data the dashboard reads from the `logs` table:
//   1. HTTP-shaped log events via the SDK's real http.Server auto-capture
//      (logger_name = 'greplog.http', attributes http.status_code /
//      http.latency_ms / ...) — this is the Node arm of the dual-source
//      StatusCodesChart and AvgLatencyByServiceChart queries.
//   2. Manual greplog.info / greplog.error events (logger_name = 'greplog')
//      — feeds the log-volume, errors, error-rate and severity charts.
//
// The HTTP server listens on an ephemeral port and is self-hitting, so no
// external traffic tool is required for the Node side.
//
// Env:
//   GREPLOG_SERVICE  service name shown in the dashboard (default api-node)
//   GREPLOG_SOCKET   UDS path; falls back to TCP 127.0.0.1:4318 if absent
//
// Run from anywhere: `node examples/dashboard-verify/node/verify-node.cjs`

const path = require('node:path')
const http = require('node:http')
const { greplog } = require(path.join(__dirname, '../../../sdks/node/dist/index.js'))

const service = process.env.GREPLOG_SERVICE || 'api-node'
const socketPath = process.env.GREPLOG_SOCKET || '.greplog/greplog.sock'

greplog.init({ service, socketPath })
console.log(`[api-node] service=${service} socket=${socketPath} (TCP fallback 127.0.0.1:4318 if UDS absent)`)

const server = http.createServer((req, res) => {
  const r = Math.random()
  let code = 200
  if (r < 0.1) code = 500
  else if (r < 0.15) code = 404
  res.writeHead(code, { 'content-type': 'application/json' })
  res.end(code === 200 ? '{"ok":true}' : '{"error":"failed"}')
})

server.listen(0, '127.0.0.1', () => {
  const port = server.address().port
  console.log(`[api-node] http server on 127.0.0.1:${port}, hitting it every 400ms`)
  setInterval(() => {
    fetch(`http://127.0.0.1:${port}/orders`).catch(() => {})
    fetch(`http://127.0.0.1:${port}/users`).catch(() => {})
  }, 400)
})

setInterval(() => {
  greplog.info('Handled request', { route: '/users', method: 'GET' })
  if (Math.random() < 0.15) {
    greplog.error('Payment failed', {
      order_id: 'ord-' + Math.floor(Math.random() * 100000),
      amount: '99.99',
    })
  }
}, 800)
