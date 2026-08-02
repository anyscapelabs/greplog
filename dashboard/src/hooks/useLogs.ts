import { useQuery } from '@tanstack/react-query'
import { useCallback, useRef } from 'react'
import type { LogsPageProps, LogEntry, LogCharts, FilterSectionConfig } from '../types/index.ts'
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

const EMPTY_CHARTS: LogCharts = {
  volumeTimeseries: [],
  errorTimeseries: [],
  statusCodeDistribution: [],
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

export function useLogs(whereClause?: string): LogsPageProps {
  const { connected } = useAgent()
  const userInitiatedRef = useRef(false)

  const queryKey = whereClause ? ['logs', whereClause] : ['logs']

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
      const [result, countResult, levelResult, serviceResult, httpStatusResult] = await Promise.all([
        postQuery(`${BASE_SQL} ${w} ORDER BY timestamp DESC LIMIT 1000`, { userInitiated }),
        postQuery(`SELECT count(*) AS total FROM logs ${w}`, { userInitiated }),
        postQuery(`SELECT level, count(*) AS cnt FROM logs ${w} GROUP BY level ORDER BY cnt DESC`, { userInitiated }),
        postQuery(`SELECT service, count(*) AS cnt FROM logs ${w} GROUP BY service ORDER BY cnt DESC`, { userInitiated }),
        postQuery(`SELECT json_get_str(attributes, 'http.status_code') AS code, count(*) AS cnt FROM logs WHERE logger_name = 'greplog.http'${andClause} GROUP BY json_get_str(attributes, 'http.status_code') ORDER BY cnt DESC`, { userInitiated }),
      ])

      const logs = result ? rowsToLogs(result.rows, result.columns) : []
      const cntIdx = countResult ? countResult.columns.indexOf('total') : -1
      const totalCount = cntIdx >= 0 && countResult && countResult.rows[0] ? Number(countResult.rows[0][cntIdx] ?? 0) : logs.length

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
        charts: EMPTY_CHARTS,
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
