import { useQuery } from '@tanstack/react-query'
import type { ErrorsPageProps, ErrorEntry, ErrorCharts } from '../types/index.ts'
import { postQuery } from './api.ts'
import {
  placeholderErrorFilterSections,
  placeholderTimeRanges,
  placeholderServices,
  placeholderAutoRefreshOptions,
  placeholderChartMetrics,
  placeholderErrorsGroupBy,
} from './placeholder-data.ts'
import { useAgent } from '../context/AgentContext.tsx'

const EMPTY_CHARTS: ErrorCharts = {
  countTimeseries: [],
  rateTimeseries: [],
  byServiceDistribution: [],
}

const errorRateGroupBy = [
  { label: 'nothing', value: 'nothing' },
  { label: 'service', value: 'service' },
  { label: 'level', value: 'level' },
  { label: 'status_code', value: 'status_code' },
]

const errorByServiceGroupBy = [
  { label: 'nothing', value: 'nothing' },
  { label: 'level', value: 'level' },
  { label: 'status_code', value: 'status_code' },
]

function rowsToErrors(rows: unknown[][], columns: string[]): ErrorEntry[] {
  const idx = (name: string) => columns.indexOf(name)
  return rows.map((r) => {
    const rawSt: unknown = r[idx('stack_trace')]
    return {
      id: String(r[idx('id')] ?? ''),
      timestamp: String(r[idx('timestamp')] ?? ''),
      errorCode: Number(r[idx('line')] ?? 0),
      freq: 1,
      level: (String(r[idx('level')] ?? 'error')) as ErrorEntry['level'],
      latency: '',
      service: String(r[idx('service')] ?? ''),
      message: String(r[idx('message')] ?? ''),
      stackTrace: Array.isArray(rawSt) ? (rawSt as string[]).join('\n') : ((rawSt as string) ?? undefined),
      errorType: r[idx('exception_type')] != null ? String(r[idx('exception_type')]) : undefined,
      correlationId: r[idx('correlation_id')] != null ? String(r[idx('correlation_id')]) : undefined,
    }
  })
}

const BASE_ERROR_FILTER = "level IN ('error','critical','fatal')"
const BASE_SQL = 'SELECT id, timestamp, level, service, message, line, stack_trace, exception_type, correlation_id FROM logs'

export function useErrors(whereClause?: string): ErrorsPageProps {
  const { connected } = useAgent()

  const queryKey = whereClause ? ['errors', whereClause] : ['errors']

  const query = useQuery({
    queryKey,
    queryFn: async () => {
      if (!connected) {
        return { errors: [], totalCount: 0, charts: EMPTY_CHARTS, isWaiting: true }
      }
      let where = `WHERE ${BASE_ERROR_FILTER}`
      if (whereClause) {
        const userWhere = whereClause.replace(/^WHERE\s+/i, '')
        where = `WHERE ${BASE_ERROR_FILTER} AND (${userWhere})`
      }
      const [result, countResult, countTimeseriesResult, totalResult, serviceResult] = await Promise.all([
        postQuery(`${BASE_SQL} ${where} ORDER BY timestamp DESC LIMIT 1000`),
        postQuery(`SELECT count(*) AS total FROM logs ${where}`),
        postQuery(`SELECT date, count(*) AS cnt FROM logs ${where} GROUP BY date ORDER BY date`),
        postQuery(`SELECT date, count(*) AS cnt FROM logs ${whereClause ?? ''} GROUP BY date ORDER BY date`),
        postQuery(`SELECT service, count(*) AS cnt FROM logs ${where} GROUP BY service ORDER BY cnt DESC`),
      ])

      const errors = result ? rowsToErrors(result.rows, result.columns) : []
      const totalCount = countResult && countResult.rows[0] ? Number(countResult.rows[0][0] ?? 0) : errors.length

      const countTimeseries = countTimeseriesResult
        ? countTimeseriesResult.rows.map((r) => ({ timestamp: String(r[0] ?? ''), value: Number(r[1] ?? 0) }))
        : []

      const totalMap = new Map<string, number>()
      if (totalResult) {
        for (const r of totalResult.rows) {
          totalMap.set(String(r[0] ?? ''), Number(r[1] ?? 0))
        }
      }

      const rateTimeseries = countTimeseries.map((pt) => {
        const total = totalMap.get(pt.timestamp) ?? pt.value
        return {
          timestamp: pt.timestamp,
          rate: total > 0 ? pt.value / total : 0,
        }
      })

      const colors = ['#ef4444', '#f59e0b', '#3b82f6', '#8b5cf6', '#10b981', '#ec4899']
      const byServiceDistribution = serviceResult
        ? serviceResult.rows.map((r, i) => ({
            name: String(r[0] ?? ''),
            value: Number(r[1] ?? 0),
            color: colors[i % colors.length],
          }))
        : []

      return {
        errors,
        totalCount,
        charts: {
          countTimeseries,
          rateTimeseries,
          byServiceDistribution,
        },
        isWaiting: false,
      }
    },
    enabled: connected,
  })

  const data = query.data ?? { errors: [], totalCount: 0, charts: EMPTY_CHARTS, isWaiting: true }

  return {
    errors: data.errors,
    totalErrors: data.totalCount,
    totalRows: data.totalCount,
    querySeconds: 0,
    filterSections: placeholderErrorFilterSections,
    charts: data.charts,
    isWaiting: data.isWaiting,
    timeRanges: placeholderTimeRanges,
    services: placeholderServices,
    autoRefreshOptions: placeholderAutoRefreshOptions,
    chartMetrics: placeholderChartMetrics,
    groupByOptions: {
      errorCount: placeholderErrorsGroupBy,
      errorRate: errorRateGroupBy,
      byService: errorByServiceGroupBy,
    },
    onViewError: undefined,
    refetch: query.refetch,
  }
}
