/**
 * Shared dual-source HTTP queries (spans UNION ALL logs.attributes) so every
 * chart that touches latency or status codes reuses ONE implementation of the
 * predicate translation (httpArmPredicates) and the SQL/parsing that follows
 * from it — never a second hand-rolled copy.
 *
 * The spans arm (Go/Rust SDKs) has typed columns; the logs arm (Node/Python
 * SDKs) carries the same request telemetry in `attributes`, extracted with
 * json_get_str on rows where logger_name = 'greplog.http'. Each arm gets its
 * own translation of the user's WHERE clause, so a filter narrows both arms
 * identically.
 */
import { httpArmPredicates } from './httpPredicates.ts'
import type { QueryResult } from '../hooks/api.ts'
import type { PieSlice } from '../types/index.ts'

export interface DualSourceSql {
  sql: string | null
  unsupported: string[]
}

function dualSource(where: string): { spansWhere: string; httpLogsAnd: string; unsupported: string[] } {
  const { spans: spansWhere, logs: httpLogsAnd, unsupported } = httpArmPredicates(where)
  return { spansWhere, httpLogsAnd, unsupported }
}

/** Per-status-code counts over the combined HTTP population. */
export function buildStatusCodesSql(where: string): DualSourceSql {
  const { spansWhere, httpLogsAnd, unsupported } = dualSource(where)
  if (unsupported.length > 0) return { sql: null, unsupported }
  return {
    sql: `
      SELECT status_code, sum(cnt) AS cnt FROM (
        SELECT CAST(status_code AS VARCHAR) AS status_code, count(*) AS cnt
        FROM spans${spansWhere} GROUP BY CAST(status_code AS VARCHAR)
        UNION ALL
        SELECT json_get_str(attributes, 'http.status_code') AS status_code, count(*) AS cnt
        FROM logs WHERE logger_name = 'greplog.http'${httpLogsAnd}
        GROUP BY json_get_str(attributes, 'http.status_code')
      ) t GROUP BY status_code ORDER BY cnt DESC`,
    unsupported: [],
  }
}

/**
 * Per-service latency (avg + percentiles) over the combined HTTP population.
 * SUM/COUNT (not AVG) so the weighted average is exact even when a service has
 * rows from both sources; the client divides.
 */
export function buildAvgLatencyByServiceSql(where: string): DualSourceSql {
  const { spansWhere, httpLogsAnd, unsupported } = dualSource(where)
  if (unsupported.length > 0) return { sql: null, unsupported }
  return {
    sql: `
      SELECT service, sum(latency_ms) AS sum_ms, count(*) AS cnt,
        approx_percentile_cont(latency_ms, 0.50) AS p50,
        approx_percentile_cont(latency_ms, 0.95) AS p95,
        approx_percentile_cont(latency_ms, 0.99) AS p99
      FROM (
        SELECT service, latency_ms FROM spans${spansWhere}
        UNION ALL
        SELECT service, CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE) AS latency_ms
        FROM logs WHERE logger_name = 'greplog.http'${httpLogsAnd}
      ) t GROUP BY service ORDER BY sum_ms / cnt DESC`,
    unsupported: [],
  }
}

/** Overall latency percentiles over the combined HTTP population. */
export function buildLatencyPercentilesSql(where: string): DualSourceSql {
  const { spansWhere, httpLogsAnd, unsupported } = dualSource(where)
  if (unsupported.length > 0) return { sql: null, unsupported }
  return {
    sql: `
      SELECT
        approx_percentile_cont(latency_ms, 0.50) AS p50,
        approx_percentile_cont(latency_ms, 0.90) AS p90,
        approx_percentile_cont(latency_ms, 0.99) AS p99
      FROM (
        SELECT latency_ms FROM spans${spansWhere}
        UNION ALL
        SELECT CAST(json_get_str(attributes, 'http.latency_ms') AS DOUBLE) AS latency_ms
        FROM logs WHERE logger_name = 'greplog.http'${httpLogsAnd}
      ) t`,
    unsupported: [],
  }
}

const STATUS_COLOR_MAP: Record<number, string> = {
  2: '#22c55e',
  3: '#3b82f6',
  4: '#f59e0b',
  5: '#ef4444',
}

export function parseStatusCodes(result: QueryResult | null): PieSlice[] {
  if (!result) return []
  const codeIdx = result.columns.indexOf('status_code')
  const cntIdx = result.columns.indexOf('cnt')
  if (codeIdx < 0 || cntIdx < 0) return []
  return result.rows
    .map((r) => {
      const code = Number(r[codeIdx] ?? 0)
      if (!Number.isFinite(code) || code === 0) return null
      return {
        name: String(code),
        value: Number(r[cntIdx] ?? 0),
        color: STATUS_COLOR_MAP[Math.floor(code / 100)] ?? '#6b7280',
      }
    })
    .filter((s): s is PieSlice => s !== null)
}

export function parseAvgLatencyByService(
  result: QueryResult | null,
): { service: string; avg: number; p50: number; p95: number; p99: number }[] {
  if (!result) return []
  const svcIdx = result.columns.indexOf('service')
  const sumIdx = result.columns.indexOf('sum_ms')
  const cntIdx = result.columns.indexOf('cnt')
  const p50Idx = result.columns.indexOf('p50')
  const p95Idx = result.columns.indexOf('p95')
  const p99Idx = result.columns.indexOf('p99')
  if (svcIdx < 0 || sumIdx < 0 || cntIdx < 0) return []
  const num = (row: unknown[], idx: number): number => {
    if (idx < 0) return 0
    const raw = row[idx]
    return raw === null || raw === undefined ? 0 : Number(raw)
  }
  return result.rows.map((r) => {
    const sum = Number(r[sumIdx] ?? 0)
    const cnt = Number(r[cntIdx] ?? 0)
    return {
      service: String(r[svcIdx] ?? ''),
      avg: cnt > 0 ? Math.round((sum / cnt) * 100) / 100 : 0,
      p50: num(r, p50Idx),
      p95: num(r, p95Idx),
      p99: num(r, p99Idx),
    }
  })
}
