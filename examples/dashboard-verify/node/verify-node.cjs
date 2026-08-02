#!/usr/bin/env node
// Manual dashboard verification — Node.js traffic generator.
//
// Emits ALL of the data the dashboard reads from the `logs` table:
//   1. HTTP-shaped log events via the SDK's real http.Server auto-capture
//      (logger_name = 'greplog.http', attributes http.status_code /
//      http.latency_ms / headers / http.request.body) — this is the Node arm
//      of the dual-source StatusCodesChart and AvgLatencyByServiceChart
//      queries. captureBodies: true so the http.request.body attribute is
//      present on every request.
//   2. Manual greplog.info / warn / error / debug events
//      (logger_name = 'greplog') — feeds the log-volume, errors, error-rate
//      and severity charts across all levels.
//   3. A periodic unhandled promise rejection, captured by the SDK's
//      unhandledRejection hook — this is the only Node path that produces
//      rows with exception_type / exception_message / stack_trace columns,
//      which feed the Errors page error-type filter and the drawer's
//      Stack Trace section.
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

greplog.init({ service, socketPath, captureBodies: true })
console.log(`[api-node] service=${service} socket=${socketPath} captureBodies=true (TCP fallback 127.0.0.1:4318 if UDS absent)`)

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
