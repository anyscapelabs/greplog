/**
 * Builders that turn aggregation query results (rows + column names) into the
 * FilterSectionConfig[] shape the filter sidebar renders. Kept as pure
 * functions in one place so the Logs/Errors/... hooks don't each hand-roll a
 * slightly different count -> item mapping. Unknown levels/statuses never
 * fabricate a value: rows whose dimension can't be parsed are skipped, and
 * buckets with zero counts are simply absent from the section.
 */
import type { FilterSectionConfig, FilterSectionItem } from '../types/index.ts'

const LEVEL_COLORS: Record<string, string> = {
  error: 'var(--error)',
  critical: '#dc2626',
  fatal: '#dc2626',
  warn: 'var(--warn)',
  info: 'var(--info)',
  debug: 'var(--text-secondary)',
}

const STATUS_BUCKETS: Record<string, { id: string; label: string; color: string }> = {
  success: { id: 'success', label: 'Success', color: '#22c55e' },
  redirect: { id: 'redirect', label: 'Redirect', color: '#3b82f6' },
  client_error: { id: 'client_error', label: 'Client Error', color: '#f59e0b' },
  server_error: { id: 'server_error', label: 'Server Error', color: '#ef4444' },
}

const ERROR_TYPE_PALETTE = ['#ef4444', '#f97316', '#eab308', '#8b5cf6', '#dc2626', '#0ea5e9']

function countIndex(columns: string[]): number {
  return columns.indexOf('cnt')
}

export function buildLevelSection(rows: unknown[][], columns: string[]): FilterSectionConfig {
  const lvlIdx = columns.indexOf('level')
  const cntIdx = countIndex(columns)
  const items: FilterSectionItem[] = rows.map((r) => {
    const level = String(r[lvlIdx] ?? '')
    return {
      id: level,
      label: level.charAt(0).toUpperCase() + level.slice(1),
      count: Number(r[cntIdx] ?? 0),
      color: LEVEL_COLORS[level] ?? undefined,
    }
  })
  return { id: 'log_level', title: 'log_level', defaultOpen: true, items }
}

export function buildServiceSection(rows: unknown[][], columns: string[]): FilterSectionConfig {
  const svcIdx = columns.indexOf('service')
  const cntIdx = countIndex(columns)
  const items: FilterSectionItem[] = rows.map((r) => {
    const service = String(r[svcIdx] ?? '')
    return { id: service, label: service, count: Number(r[cntIdx] ?? 0) }
  })
  return { id: 'service_name', title: 'service_name', defaultOpen: true, items }
}

export function buildErrorTypeSection(rows: unknown[][], columns: string[]): FilterSectionConfig {
  const typeIdx = columns.indexOf('exception_type')
  const cntIdx = countIndex(columns)
  const items: FilterSectionItem[] = rows.map((r, i) => {
    const type = String(r[typeIdx] ?? '')
    return {
      id: type,
      label: type,
      count: Number(r[cntIdx] ?? 0),
      color: ERROR_TYPE_PALETTE[i % ERROR_TYPE_PALETTE.length],
    }
  })
  return { id: 'error_type', title: 'error_type', defaultOpen: true, items }
}

/**
 * Per-status-code + bucketed response-status sections derived from one query:
 * `SELECT json_get_str(attributes, 'http.status_code') AS code, count(*) AS cnt
 *  FROM logs WHERE logger_name = 'greplog.http' ... GROUP BY code`.
 */
export function buildStatusCodeSections(
  rows: unknown[][],
  columns: string[],
): { statusCode: FilterSectionConfig; responseStatus: FilterSectionConfig } {
  const codeIdx = columns.indexOf('code')
  const cntIdx = countIndex(columns)
  const codeItems: FilterSectionItem[] = []
  const bucketCounts: Record<string, number> = { success: 0, redirect: 0, client_error: 0, server_error: 0 }
  for (const r of rows) {
    const raw = String(r[codeIdx] ?? '')
    const code = Number(raw)
    if (!raw || !Number.isFinite(code) || code <= 0) continue
    const count = Number(r[cntIdx] ?? 0)
    codeItems.push({ id: raw, label: raw, count })
    const bucket = code >= 500 ? 'server_error' : code >= 400 ? 'client_error' : code >= 300 ? 'redirect' : 'success'
    bucketCounts[bucket] += count
  }
  codeItems.sort((a, b) => b.count - a.count)
  const responseItems: FilterSectionItem[] = Object.keys(STATUS_BUCKETS)
    .map((key) => {
      const meta = STATUS_BUCKETS[key]
      return { id: meta.id, label: meta.label, count: bucketCounts[key], color: meta.color }
    })
    .filter((i) => i.count > 0)
  return {
    statusCode: { id: 'status_code', title: 'status_code', items: codeItems },
    responseStatus: { id: 'response_status', title: 'response_status', defaultOpen: true, items: responseItems },
  }
}
