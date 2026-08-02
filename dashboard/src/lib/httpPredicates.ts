/**
 * Pure SQL-predicate translation for the HTTP charts' UNION ALL queries.
 * Extracted from useAnalytics.ts so it can be unit-tested with `node --test`
 * (Node >= 23.6 runs this file directly via type stripping).
 */

/**
 * Split a SQL WHERE body on top-level ` AND ` operators, ignoring occurrences
 * inside single-quoted string literals (e.g. `message LIKE '%a AND b%'`).
 * Doubled single quotes (SQL escaping) are handled. Returns the clauses with
 * the leading `WHERE` already stripped.
 */
export function splitPredicateClauses(whereSql: string): string[] {
  const body = whereSql.replace(/^WHERE\s+/i, '').trim()
  if (!body) return []
  const clauses: string[] = []
  let buf = ''
  let inQuote = false
  for (let i = 0; i < body.length; i++) {
    const ch = body[i]
    if (ch === "'") {
      buf += ch
      if (inQuote && body[i + 1] === "'") {
        buf += "'"
        i++
      } else {
        inQuote = !inQuote
      }
      continue
    }
    if (!inQuote && body.startsWith(' AND ', i)) {
      clauses.push(buf.trim())
      buf = ''
      i += 4
      continue
    }
    buf += ch
  }
  clauses.push(buf.trim())
  return clauses.filter(Boolean)
}

/**
 * Split a comma-separated list of quoted SQL literals (e.g. the values inside
 * `level IN ('error', 'warn')`) into bare strings.
 */
export function splitQuotedList(body: string): string[] {
  return body
    .split(',')
    .map((part) => part.trim().replace(/^'(.*)'$/, '$1'))
    .filter(Boolean)
}

/**
 * Translate the user's WHERE clause into one predicate per HTTP-data arm of
 * the UNION ALL queries. The two arms store the same request telemetry in
 * different shapes, so each clause is translated per arm — a filter that
 * narrows only one arm would produce a mathematically wrong combined
 * population (e.g. a `level: error` filter restricted to the logs arm while
 * the spans arm contributed every span).
 *
 * The two shapes:
 *   - `spans` (Go/Rust SDKs): service, name, route, method, status_code
 *     (Int32), start_time/end_time, correlation_id. No `timestamp`, `level`,
 *     `message`, or `line`.
 *   - `logs` with logger_name = 'greplog.http' (Node/Python SDKs): every logs
 *     column exists. The HTTP middleware derives `level` from the response
 *     status (>=500 -> error, 400-499 -> warn, <400 -> info), so for HTTP rows
 *     `level` IS a status bucket.
 *
 * Per-clause handling:
 *   - `service IN (...)`, `correlation_id = 'x'` — unchanged on both arms.
 *   - `timestamp op to_timestamp_micros(N)` (the time-range filter) —
 *     `timestamp` (logs) / `start_time` (spans). The filter builder emits the
 *     cutoff as `to_timestamp_micros(N)` (N = micros since epoch) because
 *     DataFusion cannot coerce a bare integer literal against a
 *     `Timestamp(µs)` column.
 *   - `level IN (...)` — raw `level` on the logs arm (equivalent to a status
 *     bucket by the SDK mapping above); a `status_code` bucket predicate on the
 *     spans arm. Levels the middleware never emits (debug/critical/fatal)
 *     match nothing on both arms.
 *   - `message LIKE '%x%'` — raw `message` (logs) / `name LIKE` or `route
 *     LIKE` (spans).
 *   - `line op N` / `line = 'N'` (status chips compile to `line`) — a
 *     `status_code` predicate on both arms, since `line` is null on HTTP rows.
 *   - Unrecognized shapes — reported in `unsupported`. Keeping them on the
 *     logs arm while ignoring them on the spans arm would recreate the skewed-
 *     population bug, so the caller skips the HTTP queries entirely and warns
 *     (AGENTS.md Rule 8: fail loudly on a shape the serializer can't express).
 *
 * @returns `spans` — a ` WHERE ...` string (or '') to append to `FROM spans`;
 *          `logs` — an ` AND (...)` string (or '') for the logs arm, which
 *          already carries `WHERE logger_name = 'greplog.http'`;
 *          `unsupported` — clauses that matched no known shape. When
 *          non-empty, the caller must NOT run the HTTP queries.
 */
export function httpArmPredicates(userWhere: string): { spans: string; logs: string; unsupported: string[] } {
  if (!userWhere) return { spans: '', logs: '', unsupported: [] }
  const spans: string[] = []
  const logs: string[] = []
  const unsupported: string[] = []

  for (const clause of splitPredicateClauses(userWhere)) {
    // Columns that exist on both tables. `'.*'` tolerates SQL-escaped quotes
    // inside the correlation_id literal. Clauses arrive pre-split on top-level
    // ` AND ` (splitPredicateClauses), so the greedy `.*` is bounded to a
    // single clause and can never swallow a following predicate.
    if (/^service\s+IN\s*\(/i.test(clause) || /^correlation_id\s*=\s*'.*'$/i.test(clause)) {
      spans.push(clause)
      logs.push(clause)
      continue
    }

    // Time range: `timestamp` is a logs column; the spans time column is
    // `start_time`. The predicate arrives as `timestamp op
    // to_timestamp_micros(N)` (see useFilterState.ts); N is already micros
    // since epoch, and `to_timestamp_micros(N)` yields a Timestamp(µs)
    // directly comparable to both time columns.
    const timeMatch = clause.match(/^timestamp\s*([<>]=?)\s*to_timestamp_micros\((-?\d+)\)$/i)
    if (timeMatch) {
      spans.push(`start_time ${timeMatch[1]} to_timestamp_micros(${timeMatch[2]})`)
      logs.push(clause)
      continue
    }

    // Log severity -> status bucket on the spans arm; raw `level` on the logs
    // arm (equivalent for greplog.http rows, which carry the status-derived
    // level). Levels the HTTP middleware never emits match nothing on both
    // arms (the spans arm gets an impossible predicate; the logs arm simply
    // has no such rows).
    const levelMatch = clause.match(/^level\s+IN\s*\((.*)\)$/i)
    if (levelMatch) {
      const statusPreds: string[] = []
      for (const raw of splitQuotedList(levelMatch[1])) {
        const level = raw.toLowerCase()
        if (level === 'error' || level === 'critical' || level === 'fatal') {
          statusPreds.push('status_code >= 500')
        } else if (level === 'warn') {
          statusPreds.push('status_code >= 400 AND status_code < 500')
        } else if (level === 'info') {
          statusPreds.push('status_code < 400')
        }
      }
      spans.push(statusPreds.length > 0 ? `(${statusPreds.join(' OR ')})` : 'status_code < 0')
      logs.push(clause)
      continue
    }

    // Free-text search: the log message of an HTTP row is "METHOD route ->
    // status"; the span equivalent is name ("METHOD route") or route. The
    // quoted needle is already SQL-escaped (doubled quotes) by the filter
    // builder, so it is reused verbatim.
    const messageMatch = clause.match(/^message\s+LIKE\s+'%(.*?)%'$/i)
    if (messageMatch) {
      const needle = messageMatch[1]
      spans.push(`(name LIKE '%${needle}%' OR route LIKE '%${needle}%')`)
      logs.push(clause)
      continue
    }

    // Status chips compile to `line op N` (see useFilterState.ts). `line` is
    // null on HTTP log rows, so both arms filter on the request's status_code
    // instead — otherwise the arms would see different populations.
    const lineOpMatch = clause.match(/^line\s*([<>=!]+)\s*(\d+)$/i)
    if (lineOpMatch) {
      const op = lineOpMatch[1]
      const n = lineOpMatch[2]
      spans.push(`status_code ${op} ${n}`)
      logs.push(`CAST(json_get_str(attributes, 'http.status_code') AS INT) ${op} ${n}`)
      continue
    }
    const lineEqMatch = clause.match(/^line\s*=\s*'(\d+)'$/i)
    if (lineEqMatch) {
      spans.push(`status_code = ${lineEqMatch[1]}`)
      logs.push(`CAST(json_get_str(attributes, 'http.status_code') AS INT) = ${lineEqMatch[1]}`)
      continue
    }

    // Unrecognized shape: this is a deliberate trap. Silently keeping it on
    // the logs arm while the spans arm ignores it would recreate the skewed-
    // population bug for a future filter type. Fail loudly instead (the caller
    // skips the HTTP queries and warns), per AGENTS.md Rule 8 — an unhandled
    // predicate must be added here explicitly, never degraded silently.
    unsupported.push(clause)
  }

  const join = (parts: string[]) => (parts.length > 0 ? ` ${parts.join(' AND ')}` : '')
  const spansWhere = join(spans)
  return {
    spans: spansWhere.length > 0 ? ` WHERE${spansWhere}` : '',
    logs: logs.length > 0 ? ` AND (${logs.join(' AND ')})` : '',
    unsupported,
  }
}
