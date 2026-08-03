import { useQuery } from '@tanstack/react-query'
import { useCallback, useRef } from 'react'
import type { LogsPageProps, LogEntry, LogCharts, FilterSectionConfig, LogsHistogramGranularity } from '../types/index.ts'
import { postQuery } from './api.ts'
import {
  placeholderTimeRanges,
  placeholderServices,
  placeholderAutoRefreshOptions,
  placeholderChartMetrics,
  placeholderRequestsGroupBy,
  placeholderErrorsGroupBy,
  placeholderStatusCodesGroupBy,
} from './placeholder-data.ts'
import { useAgent } from '../context/AgentContext.tsx'
import { buildLevelSection, buildServiceSection, buildStatusCodeSections } from '../lib/filterSections.ts'
import { parseLogsHistogram } from '../lib/logsHistogram.ts'

const EMPTY_CHARTS: LogCharts = {
  volumeTimeseries: [],
  errorTimeseries: [],
  statusCodeDistribution: [],
  logsHistogram: { buckets: [], levels: [], granularity: 'minute' },
}

function rowsToLogs(rows: unknown[][], columns: string[]): LogEntry[] {
  const idx = (name: string) => columns.indexOf(name)
  return rows.map((r) => {
    const rawSt: unknown = r[idx('stack_trace')]
    return {
      id: String(r[idx('id')] ?? ''),
      timestamp: String(r[idx('timestamp')] ?? ''),
      level: (String(r[idx('level')] ?? 'info')) as LogEntry['level'],
      service: String(r[idx('service')] ?? ''),
      statusCode: Number(r[idx('line')] ?? 0),
      message: String(r[idx('message')] ?? ''),
      response: '',
      logger: String(r[idx('logger_name')] ?? ''),
      correlationId: String(r[idx('correlation_id')] ?? ''),
      file: String(r[idx('file')] ?? ''),
      stackTrace: Array.isArray(rawSt) ? (rawSt as string[]).join('\n') : ((rawSt as string) ?? undefined),
    }
  })
}

const BASE_SQL = 'SELECT id, timestamp, level, service, message, logger_name, file, line, correlation_id, stack_trace FROM logs'

// Fill missing (bucket, level) combinations with zero counts so all buckets show all levels
function fillMissingHistogramBuckets(histogram: ReturnType<typeof parseLogsHistogram>) {
  if (histogram.buckets.length === 0 || histogram.levels.length === 0) {
    return histogram
  }

  const bucketCount = histogram.buckets.length

  // Ensure each level has a count entry for every bucket
  for (const levelData of histogram.levels) {
    while (levelData.counts.length < bucketCount) {
      levelData.counts.push(0)
    }
  }

  return histogram
}

function stableQueryKeyWhereClause(whereClause?: string): string | undefined {
  if (!whereClause) return undefined
  return whereClause.replace(/to_timestamp_micros\(\d+\)/g, 'to_timestamp_micros(<now>)')
}

// Bucket granularity scales with the selected time range so the histogram
// stays readable: per-minute buckets over 7-30 days would be tens of
// thousands of near-empty bars. Ranges spanning multiple days also switch
// the chart to an area rendering (see LogsHistogramChart) since individual
// bars are meaningless at that scale.
// For 7d and 30d ranges, we use 12-hour granularity to show 2 points per day.
function histogramGranularity(timeRange?: string): LogsHistogramGranularity {
  if (timeRange === 'Last 30 days') return '12-hour'
  if (timeRange === 'Last 7 days') return '12-hour'
  return 'minute'
}

export function useLogs(whereClause?: string, timeRange?: string): LogsPageProps {
  const { connected } = useAgent()
  const userInitiatedRef = useRef(false)

  const stableWhereClause = stableQueryKeyWhereClause(whereClause)
  const granularity = histogramGranularity(timeRange)
  // `timeRange` is included explicitly (not just folded into the where
  // clause) because `stableWhereClause` intentionally normalizes away the
  // absolute `to_timestamp_micros(...)` threshold to avoid refetching on
  // every render (it changes by a few ms each time `Date.now()` is called).
  // Without this, switching from e.g. "Last 1 hour" to "Last 7 days" would
  // produce an identical query key and React Query would keep serving the
  // stale cached result instead of refetching.
  const queryKey = stableWhereClause ? ['logs', stableWhereClause, timeRange] : ['logs', timeRange]

  const query = useQuery({
    queryKey,
    queryFn: async () => {
      if (!connected) {
        return { logs: [], totalCount: 0, charts: EMPTY_CHARTS, filterSections: [], isWaiting: true }
      }
      const userInitiated = userInitiatedRef.current
      userInitiatedRef.current = false
      const w = whereClause ?? ''
      const andClause = w ? ` AND (${w.replace(/^WHERE\s+/i, '')})` : ''

      // NOTE: the top-of-page charts (Total Requests / Errors / Status Codes)
      // were removed and their data queries unwired; a replacement chart will
      // add its own queries here. Only the log rows, counts and filter-sidebar
      // sections are fetched below.
      const [result, countResult, levelResult, serviceResult, httpStatusResult, histogramResult] = await Promise.all([
        postQuery(`${BASE_SQL} ${w} ORDER BY timestamp DESC LIMIT 1000`, { userInitiated }),
        postQuery(`SELECT count(*) AS total FROM logs ${w}`, { userInitiated }),
        postQuery(`SELECT level, count(*) AS cnt FROM logs ${w} GROUP BY level ORDER BY cnt DESC`, { userInitiated }),
        postQuery(`SELECT service, count(*) AS cnt FROM logs ${w} GROUP BY service ORDER BY cnt DESC`, { userInitiated }),
        postQuery(`SELECT json_get_str(attributes, 'http.status_code') AS code, count(*) AS cnt FROM logs WHERE logger_name = 'greplog.http'${andClause} GROUP BY json_get_str(attributes, 'http.status_code') ORDER BY cnt DESC`, { userInitiated }),
        postQuery(`SELECT date_trunc('${granularity === '12-hour' ? 'hour' : granularity}', timestamp) AS bucket, level, count(*) AS cnt FROM logs ${w} GROUP BY bucket, level ORDER BY bucket, level`, { userInitiated }),
      ])

      const logs = result ? rowsToLogs(result.rows, result.columns) : []
      const cntIdx = countResult ? countResult.columns.indexOf('total') : -1
      const totalCount = cntIdx >= 0 && countResult && countResult.rows[0] ? Number(countResult.rows[0][cntIdx] ?? 0) : logs.length

      // Histogram of log volume per bucket per level, ascending. Buckets come
      // back from DataFusion as microsecond timestamps; collapse to a label
      // (time-of-day, or date + hour / date for multi-day ranges) so the
      // x-axis stays readable. Rows are grouped by (bucket, level), so pivot
      // into one counts array per level aligned to bucket order. Fill missing
      // (bucket, level) combinations with zero counts.
      const parsed = histogramResult
        ? parseLogsHistogram(histogramResult.rows, histogramResult.columns, granularity, timeRange)
        : { buckets: [], levels: [], granularity }
      const logsHistogram = fillMissingHistogramBuckets(parsed)

      const filterSections: FilterSectionConfig[] = []
      if (levelResult) filterSections.push(buildLevelSection(levelResult.rows, levelResult.columns))
      if (serviceResult) filterSections.push(buildServiceSection(serviceResult.rows, serviceResult.columns))
      if (httpStatusResult) {
        const { statusCode, responseStatus } = buildStatusCodeSections(httpStatusResult.rows, httpStatusResult.columns)
        filterSections.push(statusCode, responseStatus)
      }

      return {
        logs,
        totalCount,
        charts: {
          ...EMPTY_CHARTS,
          logsHistogram,
        },
        filterSections,
        isWaiting: false,
      }
    },
    enabled: connected,
  })

  const data = query.data ?? { logs: [], totalCount: 0, charts: EMPTY_CHARTS, filterSections: [], isWaiting: true }

  const manualRefetch = useCallback(() => {
    userInitiatedRef.current = true
    return query.refetch()
  }, [query])

  return {
    logs: data.logs,
    totalLogs: data.totalCount,
    totalRows: data.totalCount,
    querySeconds: 0,
    filterSections: data.filterSections,
    charts: data.charts,
    isWaiting: data.isWaiting,
    isFetching: query.isFetching,
    timeRanges: placeholderTimeRanges,
    services: placeholderServices,
    autoRefreshOptions: placeholderAutoRefreshOptions,
    chartMetrics: placeholderChartMetrics,
    groupByOptions: {
      requests: placeholderRequestsGroupBy,
      errors: placeholderErrorsGroupBy,
      statusCodes: placeholderStatusCodesGroupBy,
    },
    onViewLog: undefined,
    refetch: query.refetch,
    manualRefetch,
  }
}
