# Query Engine

Queries run on [Apache DataFusion](https://datafusion.apache.org/), an in-memory SQL engine built on Arrow's columnar format. Because parity with the storage engine is columnar end-to-end, Greplog answers typical log queries in milliseconds.

## Why sub-second queries

- **Columnar memory**: logs already live as Arrow `RecordBatch`es — no deserialization to query them.
- **Vectorized execution**: DataFusion processes columnar batches with SIMD-friendly kernels, not row-by-row interpreters.
- **Predicate pushdown**: Apache Parquet stores min/max statistics per page. The engine skips whole pages early in time-range and level filters, reading only the bytes that match.
- **Parallelism**: queries fan out across CPU cores via DataFusion's execution plan, and the compactor keeps file counts low enough that scans stay cheap.

## Example queries

```sql
-- Count errors per service in the last hour
SELECT service, count(*) AS n
FROM logs
WHERE level = 'ERROR'
  AND timestamp_us >= now() - interval '1 hour'
GROUP BY service
ORDER BY n DESC;
```

```sql
-- Find the slowest payment requests today
SELECT trace_id, message
FROM logs
WHERE service = 'payment-service'
  AND timestamp_us >= current_date
ORDER BY timestamp_us DESC
LIMIT 20;
```

## SSE live tail

The live-tail stream does **not** hit DataFusion. It reads committed records straight from the shared live buffer and pushes them over Server-Sent Events (`GET /api/tail`) to subscribed dashboards, keeping tail latency at the microsecond scale while interactive queries reuse the full engine.

## Supported set

- `SELECT`, filtering, aggregation, window functions
- Joins, including across services within the unified `logs` view
- Standard SQL types: timestamps, strings, numbers, JSON
- Read-only guard: only a single `SELECT`/`WITH` query is accepted per request; DDL, `COPY`, `INSERT`, `DELETE`, and `SET` are rejected, and each query is subject to a timeout and a result row cap

Anything DataFusion supports, Greplog queries support.