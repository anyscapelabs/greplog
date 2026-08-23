# Security Policy

## Reporting

Email **security@greplog.dev** rather than opening a public issue. You will
get an acknowledgment within 72 hours and a fix timeline once confirmed.

## Scope notes

- The query API only executes read-only `SELECT` statements; anything else
  is rejected before reaching the engine. If you find a bypass, that is in
  scope.
- The dashboard binds to `0.0.0.0` without authentication by design — it is
  meant for localhost or a private network. Put a reverse proxy with auth in
  front of it before exposing one. Reports about missing auth on a
  publicly-bound instance are still welcome, but hardening guidance is the
  expected outcome.
- Ingest (`POST /api/log`) accepts batches up to 5 MB; payload-level DoS
  reports should include a reproduction.

## Supported versions

Only the latest `main` receives security fixes during the 0.x series.
