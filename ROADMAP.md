# Greplog Roadmap

This is the **single** roadmap for Greplog. It has two views, both kept in
this one document:

- **Version-phased view** (below) — answers "what does v0.3.0 mean."
- **Per-component status table** (further down) — answers "is the Errors page done yet."

Keep the two in sync: when a component in the status table ships, check
whether it completes a milestone in the version view below.

> **Status:** 🚧 In progress — active phase is `0.1.x` (Alpha hardening); `0.1.0` shipped.

## Versioning Discipline

Following semver conventions strictly, since other devs and early adopters
will read version numbers as promises:

- **Patch (`0.1.x`)** — bug fixes, UI polish, performance work, doc fixes.
  No new user-facing capability. Everything done since the v0.1.0 alpha
  (filter wiring, chart data-reality fixes, service health, SDK DX cleanup)
  falls in this bucket — it's hardening the alpha, not adding new features.
- **Minor (`0.x.0`)** — new capability, backward compatible. New SDK
  languages, new deployment modes, new dashboard features.
- **Major (`x.0.0`)** — reserved for `1.0.0` (stability/GA commitment) and
  any future breaking change to the wire protocol, storage format, or CLI
  interface significant enough to require migration.

## Phase Overview

| Version | Name | Scope |
|---|---|---|
| `0.1.0` | **Alpha — local dev only** | Single-node, local-first, no auth, no self-hosting. Current state. |
| `0.1.x` | **Alpha hardening** | Bug fixes, dashboard wiring correctness, chart data-reality, SDK DX. In progress now. |
| `0.2.0` | **Expanded SDK language support** | Additional language SDKs beyond the initial four. |
| `0.3.0` | **Self-hosted / remote mode** | Auth, login, API keys, custom domain, multi-user. Not a hosted SaaS product — see note below. |
| `0.4.0` | **Settings & retention** | Configurable data retention, per-workspace settings surface. |
| `0.5.0` | **External storage backends** | S3/R2/GCS support via a storage abstraction layer. |
| `0.6.0+` | **Suggested next features** | See §7 — alerting, OTLP ingestion, distributed tracing, etc. Not yet committed, proposed for consideration. |
| `1.0.0` | **General availability** | Stability commitment — see §8 for exit criteria. |

---

## 1. `0.1.0` — Alpha (Current)

Local-first, single developer or small team, no infrastructure. This is
what's shipped and described in the current `README.md`. No auth, no
multi-user concept, no remote deployment — `greplog dev` on localhost only.

## 2. `0.1.x` — Alpha Hardening (In Progress)

Everything currently being worked on: dashboard filter/chart wiring,
service health computation, drawer real-data fixes, SDK DX unification,
query-engine correctness fixes. **No new capability ships in this range** —
only fixes and polish to what `0.1.0` already promised. Once this range is
stable (no major open bugs, the per-component table below mostly ✅),
that's the signal to start `0.2.0`.

**Positioning change, in progress:** the product pitch is narrowing from
general "observability" to **backend services specifically**. `README.md`'s
tagline, use cases, and architecture diagram have been updated (backend
service examples, no frontend-app framing). **Remaining doc sweep — tracked
in the new Documentation section below**, not yet closed.

**Framework auto-detection expansion** (within the four already-shipped
SDKs, distinct from `0.2.0`'s new *languages*) — tracked as its own backlog
in the SDK section below.

## 3. `0.2.0` — Expanded SDK Language Support

Additional SDKs beyond the initial Node/Python/Go/Rust set. Candidates,
roughly in likely-priority order based on ecosystem size and overlap with
the target audience:

- **Java/Kotlin** (Spring Boot is extremely common in the small-team backend
  space)
- **.NET/C#** (ASP.NET Core — large ecosystem, currently unaddressed)
- **Ruby** (Rails — still a common small-team default)
- **PHP** (Laravel — large install base, often underserved by newer
  observability tools)

**Before starting:** apply the same shared-contract discipline from
`docs/sdk/design.md` §1 to each new language — transport, redaction, manual
API shape, fail-open guarantee, and the single-import DX all need to hold
for every new SDK from day one, not be retrofitted after. Recommend also
resolving the Node/Python wire-format reconciliation item already flagged
in the SDK section below (HTTP data as `Span` vs. `LogEvent.attributes`)
before adding more SDKs, so the pattern every new language follows is
already consistent.

## 4. `0.3.0` — Self-Hosted / Remote Mode (Not "Cloud")

**Naming clarification, confirmed:** this is **self-hosted remote
deployment** ("remote" mode), not a managed SaaS product. **"Cloud" is
reserved** for a possible future managed offering (Greplog operating
infrastructure for customers, with its own billing/multi-tenancy/SLA
scope) — that's explicitly not this phase. See §7 for the placeholder note.

**Scope:**
- New deployment mode: `greplog remote` (or equivalent — e.g. `greplog
  serve --domain logs.example.com`), distinct from `greplog dev` — binds to
  a real network interface/domain instead of localhost-only.
- **Authentication:** login-gated dashboard access. User accounts stored in
  **SQLite** — a new persistence layer, separate from the Arrow/Parquet log
  storage, needing its own ADR (does not conflict with the existing "no
  embedded OLAP database" decision, which was scoped to log data
  specifically).
- **API keys: created in the dashboard, not the CLI.** Key management
  (creation, scoping, revocation) belongs in the same UI where a user
  manages their account and permissions. Local `greplog dev` mode remains
  keyless/zero-config — API keys are a remote-mode-only concept.
- **Scoped to dashboard-level permissions** — keys carry permission scopes
  (e.g., ingest-only vs. ingest+query vs. admin), managed alongside
  whatever role/permission model `0.3.0` establishes for users generally.

## 5. `0.4.0` — Settings & Retention

- Configurable data retention (auto-delete partitions older than N days) —
  builds directly on the existing compaction/partition system
  (`docs/architecture/agent-pipeline.md`), which already understands partition
  boundaries. This is a natural extension, not new infrastructure.
- A settings surface in the dashboard (or CLI) for retention window,
  redaction rule configuration (currently code-level defaults per
  `docs/sdk/design.md` §1.3), and whatever else accumulates a need for
  configuration by this point.

## 6. `0.5.0` — External Storage Backends (S3/R2/GCS)

**This needs a storage abstraction layer, not three separate integrations.**
Today, flush/compaction write directly to local paths
(`~/.greplog/<project-hash>/logs/...`, per `docs/architecture/agent-pipeline.md`'s
File Layout). Supporting object storage means:

- Defining a storage trait/interface (`put`, `get`, `list`, atomic
  rename-or-equivalent for the existing write-to-`.tmp`-then-rename
  pattern) that the local filesystem implementation already satisfies.
- Implementing that same interface against S3-compatible APIs (AWS S3,
  Cloudflare R2, GCS via its S3-compatible endpoint or native SDK) — one
  abstraction, multiple backends, not three bespoke integrations.
- **Compaction and WAL behavior need re-validation against network storage
  latency** — the existing correctness invariants assumed local disk I/O
  latency (~57-103μs). Object storage round-trips are orders of magnitude
  slower; confirm the WAL/flush timing assumptions still hold or need
  adjustment for this backend, rather than assuming the existing
  performance characteristics transfer unchanged.

## 7. Suggested Additional Features (Not Yet Committed)

Proposed for consideration, roughly ordered by how directly they build on
what already exists vs. how much new ground they'd break:

- **Alerting/webhooks** — notify (Slack/Discord/email/webhook) on error-rate
  spikes or pattern matches. Builds directly on the health/error-rate
  computation already implemented — the aggregation exists, this adds a
  threshold-triggered notification on top of it.
- **Native OTLP ingestion** — accept standard OpenTelemetry protocol
  logs/traces directly, so users with existing OTel instrumentation can
  point at Greplog without installing a Greplog-specific SDK. Potentially
  high-leverage for adoption.
- **Distributed tracing (W3C trace-context propagation)** — this is what
  ADR-0004 actually deferred (not the lightweight HTTP `Span` capture
  already shipped). Worth revisiting once `0.2.0`/`0.3.0` are stable.
- **Log-based alert rules** — "notify if error rate > X% for Y minutes,"
  building on the same health computation as alerting above.
- **Structured-logging-library transports** — first-class Winston/Pino
  transports (Node), `structlog` integration (Python).
- **Team roles/permissions** — once `0.3.0`'s auth exists, admin vs.
  viewer roles for self-hosted multi-user deployments.
- **SSO (OIDC/SAML)** — later-stage enterprise self-hosted requirement.
- **CI/CD log capture** — point a CI pipeline at a short-lived Greplog
  agent instance to capture test-run logs/failures.
- **Audit logging for self-hosted deployments** — who queried what, once
  multi-user auth exists.
- **Managed hosting ("Greplog Cloud")** — a fully-hosted SaaS offering
  where Greplog operates infrastructure for customers, distinct from
  self-hosted remote mode (`0.3.0`). Name reserved, not scoped — would need
  its own roadmap phase, business model, and billing/multi-tenancy design
  if pursued.
- **In-app toast notifications (error/success)** — a full spec exists
  (top-right, red/green, anti-spam dedupe rules) but is **not implemented**
  (confirmed via repo-wide search — zero hits). Originally scoped to
  surface silent query failures to end users, not just to developers via
  console. Status: shelved pending a decision on whether the problem it
  solves is still live in practice, or whether existing empty-state UI +
  console logging has proven sufficient. Revisit explicitly rather than
  letting the spec quietly expire unaddressed.

## 8. `1.0.0` — General Availability: Exit Criteria

Not scheduled by date — gated on:
- `0.1.x` through `0.5.0` shipped and stable (no major open correctness
  bugs across auth, storage, retention).
- Wire protocol and storage format considered stable enough to commit to
  backward compatibility going forward.
- Documentation (per `docs/style-guide.md`) fully current against actual
  shipped behavior — no known status-marker drift.

---

# Per-Component Status Table

## Legend

- ✅ **Shipped** — complete and tested
- 🚧 **In progress** — actively being worked on
- 📋 **Planned** — designed but not started
- ⏸️ **On hold** — deliberately deprioritized, not scheduled; kept visible rather than deleted so the decision isn't silently lost

## Agent

| Feature | Priority | Status |
|---------|----------|--------|
| Ingest acknowledgment (`IngestResponse`) | High | ✅ Done |
| Crash-safe WAL | High | ✅ Done |
| Content-based dedup (BLAKE3) | High | ✅ Done |
| Parquet flush + compaction | High | ✅ Done |
| Query engine (in-memory buffer + Parquet reads) | High | ✅ Done |
| HTTP query API | High | ✅ Done |
| UDS + TCP ingest | High | ✅ Done |
| Framework workspace detection (/detect) | High | ✅ Done |
| System resources endpoint (/resources) | High | ✅ Done |
| Query engine aggregation (`GROUP BY`/`COUNT`/`SUM`, `json_get_str` on attributes) | High | ✅ Done |
| Metric aggregation & rollups | Medium | 📋 Pending |
| Span trace tree queries | Medium | 📋 Pending — needed only if Traces page (below) is reactivated |
| Retention policies | Medium | 📋 Pending — targeted for `0.4.0` |
| More language detection | Medium | 📋 Pending |
| Compression options (Zstd/snappy) | Medium | 📋 Pending |
| TLS support | Low | 📋 Pending |
| Auth / API keys | Low | 📋 Pending — targeted for `0.3.0` |

## CLI

| Feature | Priority | Status |
|---------|----------|--------|
| `greplog dev` (start agent) | High | ✅ Done |
| `greplog status` (health check) | High | ✅ Done |
| `greplog init` (framework detection) | High | ✅ Done |
| Port conflict detection | Low | 📋 Pending |
| Graceful shutdown timeout | Low | 📋 Pending |

## Core

| Feature | Priority | Status |
|---------|----------|--------|
| Protobuf schema + generated code | High | ✅ Done |
| Arrow schema definitions | High | ✅ Done |
| PII redaction | High | ✅ Done |
| ULID generation | High | ✅ Done |

## Documentation

Tracked explicitly — these were identified as needed during the backend-only
positioning change and elsewhere, but weren't previously in a trackable
table.

| Item | Priority | Status |
|------|----------|--------|
| `README.md` tagline, use cases, architecture diagram → backend-only framing | High | ✅ Done |
| `docs/architecture/overview.md` system-map diagram — replace frontend-app example with a backend service example | Medium | ✅ Done — "Node App (UI)" → "Node API", tagline updated |
| `docs/architecture/dashboard.md` Unified Timeline description — drop "frontend requests" framing | Medium | ✅ Done — now reads "aligns requests and responses across services" |
| `docs/sdk/auto-detection.md` — explicit decision on whether Next.js stays in scope under the backend-only pitch (it runs real server-side code via API routes/SSR, so there's a legitimate case either way) | Medium | 📋 Pending — needs a decision, not an automatic removal |
| ESLint backlog: 26 problems (25 errors + 1 warning) in untouched files (`AgentContext.tsx`, `ThemeContext.tsx`, `Errors.tsx`, `Logs.tsx`, `Services.tsx`) | Low | 📋 Pending — flagged twice, not yet scheduled; worth its own short cleanup round |

## Dashboard

| Feature | Priority | Status |
|---------|----------|--------|
| Global filter bar (URL-synced FilterState) | High | ✅ Done |
| Connect to real agent API | High | ✅ Done |
| Live/refresh/auto-refresh unified mechanism | High | ✅ Done |
| No fabricated chart data (honest empty states) | High | ✅ Done |
| Analytics: ingestion, error rate, service health, noisy services, severity | High | ✅ Done (queries + server-side aggregation) |
| Analytics metrics (error rate %, active services, unhealthy count, total events) | High | ✅ Done — all computed server-side |
| `totalLogs`/`totalErrors` correct server-side count (not paginated slice length) | High | ✅ Done |
| Metrics computed server-side (error rate ratio, healthy count) | High | ✅ Done — no frontend ratio computation |
| Analytics charts: latency percentiles, status codes, avg response time | Medium | ✅ Done — dual-source: `spans` table (Go/Rust) `UNION ALL` `logs.attributes` via `json_get_str()` (Node.js/Python), combined server-side so percentiles/sums/counts are computed over the full population, not per-SDK. Filter predicates translated per arm (`timestamp`→`start_time`, `level`→status-code bucket, `message LIKE`→`name`/`route LIKE`, `line`→`status_code`). Unrecognized filter shapes fail loud (HTTP queries skipped, `console.warn`) rather than silently narrowing one arm and not the other. |
| Analytics chart: system metrics (CPU, memory, disk, network) | Low | ✅ Done — agent `system.rs` collector extended with per-interface `rx_bytes_per_sec`/`tx_bytes_per_sec` (sysinfo `Networks` deltas); `GET /resources` returns them; dashboard `SystemMetricsChart` renders 4 real gauges from a 5s-polled `resourcesQuery` |
| Logs page charts (LogVolume, Errors, StatusCodes) | Medium | ✅ Done — `LogVolume`/`Errors` timeseries aggregated per-date; **`StatusCodes` uses the dual-source (`spans` `UNION ALL` `attributes`) treatment** via the shared `lib/httpQueries.ts` module, so Node/Python HTTP status codes render instead of silently showing empty |
| Errors page charts (ErrorCount, ErrorRate, ErrorByService) | Medium | ✅ Done — per-date error counts, error rate vs. all-log total, per-service distribution from the existing `useErrors` queries |
| Services page charts (RequestsByService, ErrorRateByService) | Medium | ✅ Done — real event counts + derived error rates from the health query, metric-aware (count/rate) chart props |
| Services page chart: AvgLatencyByService | Medium | ✅ Done — **dual-source (`spans` `UNION ALL` `logs.attributes` via `json_get_str`)**, reusing `buildAvgLatencyByServiceSql` from `lib/httpQueries.ts` (same predicate translation as Analytics), windowed by the Services time range |
| Errors page (wired filtering) | Medium | ✅ Done |
| Services page (sidebar filtering) | Medium | ✅ Done |
| Service Cards: sparklines from time-bucketed queries | Medium | ✅ Done |
| ServicesDrawer: Recent Errors from `useErrors` + Related Logs from `useLogs` | Medium | ✅ Done |
| Service Details: honest "Streaming since" proxy from `MIN(timestamp)` | Medium | ✅ Done |
| Sidebar filter real counts from query | Medium | 📋 Next — aggregation groundwork now in place; requires `GROUP BY level`, `GROUP BY service` queries wired into filter sidebar |
| Service version/hostname in Service Details | Low | 📋 Pending — requires cross-SDK protocol change (new handshake field) |
| Traces page | Medium | ⏸️ **On hold — deprioritized, not scheduled.** Requires span trace-tree query support (Agent table, above) which isn't being pursued right now. Kept visible rather than deleted; revisit if/when distributed tracing (§7) is picked up. |
| Views page (saved filters) | Medium | ⏸️ **On hold — deprioritized, not scheduled.** Still blocked on the unresolved question of whether saved-view persistence needs a new agent-side CRUD surface, per earlier discovery — not being pursued right now. |
| Patterns page (log pattern detection) | Low | 📋 Pending |
| Live tail (SSE streaming) | Low | 🚧 In progress (endpoint shipped, dashboard not yet consuming) |
| Chart click-to-filter | Low | 📋 Planned (deferred to post-Round 15) |
| In-app toast notifications | Low | 📋 Spec written, not implemented — see §7. Not started, not scheduled; decision pending on whether it's still needed. |

## SDKs

| Language | Status |
|----------|--------|
| Node.js | ✅ Shipped |
| Python | ✅ Shipped |
| Go | ✅ Shipped |
| Rust | ✅ Shipped |

### SDK wire-format reconciliation (backlog)

HTTP request/response capture is semantically the same data across all four SDKs,
but it is currently sent two different ways on the wire:

- **Go and Rust SDKs** → first-class `Span` messages (`IngestBatch.spans`) with typed
  `method` / `route` / `status_code` / `start_time_ns` / `end_time_ns` fields.
- **Node.js and Python SDKs** → `LogEvent.attributes` (`IngestBatch.logs`) as string
  keys `http.method` / `http.route` / `http.status_code` / `http.latency_ms` with
  `logger_name = "greplog.http"`.

This violates the sdk-design principle that the same semantic data must not vary per
language. The dashboard now reads both representations (spans table + `json_get_str`
on attributes) via a shared dual-source query pattern, which is a stopgap, not the
permanent design. **Every current and future HTTP-derived chart must use this
dual-source pattern, not a spans-only query, until this reconciliation lands.**

| Task | Priority | Status |
|------|----------|--------|
| Reconcile HTTP capture onto one wire representation (align Node.js/Python SDKs to send `Span` messages, matching Go/Rust) | Medium | 📋 Planned — needs cross-SDK protocol change (all 4 SDKs) + agent-side verification; dashboard dual-source queries remain a safe fallback until then |

### Framework auto-detection expansion (backlog)

Additive detection-signature work *within* the four already-shipped SDKs —
distinct from `0.2.0`'s new *languages*. Reasonable to land during `0.1.x`
hardening or as a short-lived adjacent effort.

| Language | Currently detected | Candidates to add |
|---|---|---|
| Node | Express, Fastify, NestJS, Next.js | Koa, Hapi |
| Python | Flask, Django, FastAPI | Tornado, Sanic |
| Go | Gin, Echo, Fiber | Chi, Gorilla Mux |
| Rust | Axum, Actix | Rocket, Warp |

## Distribution

| Feature | Priority | Status |
|---------|----------|--------|
| Shell-script installer (`curl \| sh`) downloading precompiled binary | High | ✅ Done |
| Precompiled binaries (Linux, macOS) | High | ✅ Done |
| Homebrew formula | Medium | 📋 Pending |
| Windows native (non-WSL2) | Low | 📋 Pending |
| npm package (CLI) | Low | ⏸️ On hold — removed, superseded by the shell-script installer (ADR-0010); no npm wrapper exists for the CLI. Revisit only via a new ADR if npm-based CLI distribution is ever genuinely required. |
| `get.greplog.dev` redirect for the installer one-liner | Low | 📋 Pending — `install.sh` is served from the raw GitHub URL until a domain is available (follow-up, not implemented now) |

## Performance (baseline)

Ingest throughput baseline (v0.1): ~16.7k ev/s single producer, ~25k ev/s multi-producer (32 producers). See [`bench/`](bench/) for methodology and machine config. Performance optimization is deferred — correctness first.

---

## Keeping the Two Views in Sync

Per `docs/style-guide.md`: when a per-component item ships, check whether it
completes a milestone in the version-phased view above and update this
document in the same change — don't let the two views drift into
disagreement about what's actually done.
