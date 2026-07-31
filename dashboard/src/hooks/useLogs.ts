import { useQuery } from '@tanstack/react-query'
import type { LogsPageProps, LogEntry, LogCharts } from '../types/index.ts'
import { postQuery } from './api.ts'
import {
  placeholderLogFilterSections,
  placeholderTimeRanges,
  placeholderServices,
  placeholderAutoRefreshOptions,
  placeholderChartMetrics,
  placeholderRequestsGroupBy,
  placeholderErrorsGroupBy,
  placeholderStatusCodesGroupBy,
} from './placeholder-data.ts'
import { useAgent } from '../context/AgentContext.tsx'

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

  const queryKey = whereClause ? ['logs', whereClause] : ['logs']

  const query = useQuery({
    queryKey,
    queryFn: async () => {
      if (!connected) {
        return { logs: [], totalCount: 0, charts: EMPTY_CHARTS, isWaiting: true }
      }
      const w = whereClause ?? ''
      const andClause = w ? ` AND (${w.replace(/^WHERE\s+/i, '')})` : ''
      const [result, volResult, errResult, countResult] = await Promise.all([
        postQuery(`${BASE_SQL} ${w} ORDER BY timestamp DESC LIMIT 1000`),
        postQuery(`SELECT date, count(*) AS cnt FROM logs ${w} GROUP BY date ORDER BY date`),
        postQuery(`SELECT date, count(*) AS cnt FROM logs WHERE level IN ('error','critical','fatal')${andClause} GROUP BY date ORDER BY date`),
        postQuery(`SELECT count(*) AS total FROM logs ${w}`),
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

      return {
        logs,
        totalCount,
        charts: {
          volumeTimeseries,
          errorTimeseries,
          statusCodeDistribution: [],
        },
        isWaiting: false,
      }
    },
    enabled: connected,
  })

  const data = query.data ?? { logs: [], totalCount: 0, charts: EMPTY_CHARTS, isWaiting: true }

  return {
    logs: data.logs,
    totalLogs: data.totalCount,
    totalRows: data.totalCount,
    querySeconds: 0,
    filterSections: placeholderLogFilterSections,
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
  }
}
