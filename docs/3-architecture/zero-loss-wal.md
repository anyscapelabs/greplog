# Zero-Loss Write Ahead Log

Greplog guarantees **zero data loss** even if the server crashes mid-ingest. This is enforced by a group commit pipeline over a segment-scoped WAL.

## The pipeline

```
[SDK Ingest] ➔ [Axum Server] ➔ [MPSC Funnel] ➔ [WAL fsync] ➔ [Arrow MemTable]
```

1. The SDK buffers log records and batches them to `POST /api/log`.
2. The Axum server receives the batch and pushes records into an MPSC channel — a funnel that coalesces concurrent requests.
3. The engine appends incoming records to the active `current.wal` segment.
4. The engine performs an `fsync` on the WAL file.
5. Only **after fsync succeeds** does the server acknowledge `HTTP 200 OK` back to the SDK.

## Why fsync before ack

Accepting the HTTP 200 before the WAL is durable would mean acknowledging a log you could lose. By fsyncing first, Greplog guarantees that once the SDK receives 200 OK, that record is on stable storage and can be replayed after any crash.

## Group commit

Multiple concurrent batches arriving while an fsync is in flight are folded into the same fsync call. Instead of one disk sync per request, the engine amortizes many requests into a single sync — this is how we keep CPU and disk overhead under ~3%.

## Recovery

On startup, Greplog replays every WAL segment — sealed files oldest-first, then the active `current.wal` — back into the live buffer so no committed record is ever missing from queries, then proceeds with normal ingestion. Segments are never wholesale-truncated: once a segment's records are durably stored as Parquet, it is rotated to a `sealed-<n>-*.wal` file and only reclaimed later when every record it covers has been flushed.

## Durability vs. latency trade-off

If you accept slightly stronger latency guarantees on very high write rates, fsync-on-every-batch can become the bottleneck. The default `greplog start` configuration batches aggressively (client + funnel + group commit) so throughput stays high without sacrificing durability.