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
import { buildStatusCodesSql, parseStatusCodes } from '../lib/httpQueries.ts'
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

      // Status-code distribution must come from the same dual-source HTTP
      // population as the Analytics page (spans UNION ALL logs.attributes),
      // so this page shows the same real data for mixed-SDK workspaces instead
      // of a second hand-rolled implementation. Unsupported predicate shapes
      // leave the chart empty (honest empty state), mirroring Analytics.
      const statusCodesDual = buildStatusCodesSql(w)
      if (statusCodesDual.unsupported.length > 0) {
        console.warn(
          `[logs] skipping status-code chart: ${statusCodesDual.unsupported.length} filter clause(s) ` +
            `cannot be translated to the spans/logs-attributes tables. Add a case to httpArmPredicates ` +
            `in httpPredicates.ts. Unsupported clause(s): ${statusCodesDual.unsupported.join('; ')}`,
        )
      }

      const [result, volResult, errResult, countResult, statusCodeResult, levelResult, serviceResult, httpStatusResult] = await Promise.all([
        postQuery(`${BASE_SQL} ${w} ORDER BY timestamp DESC LIMIT 1000`, { userInitiated }),
        postQuery(`SELECT date, count(*) AS cnt FROM logs ${w} GROUP BY date ORDER BY date`, { userInitiated }),
        postQuery(`SELECT date, count(*) AS cnt FROM logs WHERE level IN ('error','critical','fatal')${andClause} GROUP BY date ORDER BY date`, { userInitiated }),
        postQuery(`SELECT count(*) AS total FROM logs ${w}`, { userInitiated }),
        !statusCodesDual.sql ? Promise.resolve(null) : postQuery(statusCodesDual.sql, { userInitiated }),
        postQuery(`SELECT level, count(*) AS cnt FROM logs ${w} GROUP BY level ORDER BY cnt DESC`, { userInitiated }),
        postQuery(`SELECT service, count(*) AS cnt FROM logs ${w} GROUP BY service ORDER BY cnt DESC`, { userInitiated }),
        postQuery(`SELECT json_get_str(attributes, 'http.status_code') AS code, count(*) AS cnt FROM logs WHERE logger_name = 'greplog.http'${andClause} GROUP BY json_get_str(attributes, 'http.status_code') ORDER BY cnt DESC`, { userInitiated }),
      ])

      const logs = result ? rowsToLogs(result.rows, result.columns) : []
      const cntIdx = countResult ? countResult.columns.indexOf('total') : -1
      const totalCount = cntIdx >= 0 && countResult && countResult.rows[0] ? Number(countResult.rows[0][cntIdx] ?? 0) : logs.length

      const volumeTimeseries = volResult
        ? volResult.rows.map((r) => ({ timestamp: String(r[0] ?? ''), value: Number(r[1] ?? 0) }))
        : []
      const errorTimeseries = errResult
        ? errResult.rows.map((r) => ({ timestamp: String(r[0] ?? ''), count: Number(r[1] ?? 0) }))
        : []

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
          volumeTimeseries,
          errorTimeseries,
          statusCodeDistribution: statusCodeResult ? parseStatusCodes(statusCodeResult) : [],
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
