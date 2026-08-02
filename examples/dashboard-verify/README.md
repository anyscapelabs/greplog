# Manual Dashboard Verification — Traffic Harness

Two test apps that generate the traffic conditions the manual verification
test plan needs: a **Node** app (emits logs — the `logs.attributes` arm of the
dual-source HTTP queries) and a **Go** app (emits real HTTP spans through the
SDK middleware — the `spans` arm). Run both simultaneously for 3–5 minutes,
then walk the per-chart checklist. Both SDKs fail open and fall back from UDS
to `TCP 127.0.0.1:4318`, so the apps connect to the agent from any directory.

## 0. Prereqs

- Agent binary built: `cargo build -p greplog-agent` (or the `greplog` CLI).
- Node SDK built: `sdks/node` `dist/` already present (rebuild with
  `cd sdks/node && npm run build` if you changed `src/`).
- Go SDK: vendored via `replace` — `go run` fetches deps once (network).

## 1. Start the agent from a clean directory

Use an empty dir so no stale data pollutes the charts:

```sh
mkdir -p /tmp/greplog-verify && cd /tmp/greplog-verify
greplog dev --foreground          # or: target/debug/greplog-agent --workspace /tmp/greplog-verify --ui-port 4317 --tcp-port 4318
```

Leave it running. Open http://localhost:4317.

## 2. Start the Node app (terminal 2)

```sh
node /path/to/greplog/examples/dashboard-verify/node/verify-node.cjs
```

It runs an internal HTTP server and self-hits it every 400ms (emitting
`greplog.http` log events) plus manual `info`/`error` logs every 800ms
(~15% errors). Service name: `api-node` (override with `GREPLOG_SERVICE`).

## 3. Start the Go app (terminal 3)

```sh
cd /path/to/greplog/examples/dashboard-verify/go
go run .                          # service go-api, listens on 127.0.0.1:8080
```

## 4. Drive traffic at the Go app (terminal 4) — REQUIRED

The middleware only captures requests that actually happen:

```sh
/path/to/greplog/examples/dashboard-verify/traffic.sh        # hits /orders every 0.3s, ~10% HTTP 500
```

## 5. Verify connection

Both `api-node` and `go-api` must show as connected (WaitingOverlay cleared,
neither listed `detected_only`) before checking charts.

## 6. Walk the per-chart checklist (see the test plan)

Key conditions the harness produces:

- **StatusCodesChart**: 200s + 500s from **both** apps (spans + attributes
  merged). If you only see one language's codes, the dual-source merge is
  broken.
- **AvgLatencyByServiceChart**: non-zero latency for **both** `go-api`
  (spans arm, tens of ms) and `api-node` (attributes arm, ~1ms).
- **ErrorRateByService**: different rates per service (Go ~10%, Node ~15%).
- **Errors page / severity / noisy services**: Node's manual `error` logs
  plus both apps' HTTP 500s.
- **Toast regression**: with the agent down, click manual Refresh while a
  background error toast is visible — a second, independent error toast must
  appear (the dedupe-bypass fix).

## Env overrides

| Var | Node | Go | Default |
|---|---|---|---|
| `GREPLOG_SERVICE` | ✓ | ✓ | `api-node` / `go-api` |
| `GREPLOG_SOCKET` | ✓ | ✓ | `.greplog/greplog.sock` (TCP fallback :4318) |
| `PORT` | — | ✓ | `8080` |

## Cleaning up

Ctrl+C each app, then the agent. `greplog.config.json` (written by the Go
SDK's framework detection) and the built binary are gitignored.
