# Resolved Architectural Decisions (Locked In)

- **Wire format:** Protobuf. Safer default for ecosystem support across Go/Python/Node/Rust and allows strict schema evolution.
- **Windows support:** v1 is Unix-only (macOS/Linux + WSL2). Native Windows named pipes are deferred.
- **Agent Scope:** One agent per workspace (monorepo multiplexing), binding to a local `.greplog/greplog.sock` and local TCP fallback port.
- **Distributed Tracing:** Deferred. v1 relies on chronological interleaving (pseudo-tracing) + optional manual `correlation_id` to keep SDKs thin and stable. W3C trace propagation will come in a later phase.
- **DuckDB Writes:** Strictly single-threaded writer loop fed by channels to avoid contention, with concurrent reads enabled for the dashboard.
- **SDK Startup:** Fail-open with exactly one warning log, then silent drops.
