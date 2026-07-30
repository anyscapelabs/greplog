import { useQuery } from '@tanstack/react-query'
import type { ErrorsPageProps, ErrorEntry, ErrorCharts } from '../types/index.ts'
import { postQuery } from './api.ts'
import {
  placeholderErrors,
  placeholderErrorFilterSections,
  placeholderErrorCharts,
  placeholderTimeRanges,
  placeholderServices,
  placeholderAutoRefreshOptions,
  placeholderChartMetrics,
  placeholderErrorsGroupBy,
} from './placeholder-data.ts'
import { useAgent } from '../context/AgentContext.tsx'

const MOCK_ERRORS = placeholderErrors(50000)
const MOCK_CHARTS = placeholderErrorCharts()

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

function rowsToErrors(rows: string[][], columns: string[]): ErrorEntry[] {
  const idx = (name: string) => columns.indexOf(name)
  return rows.map((r) => ({
    id: r[idx('event_id')] ?? '',
    timestamp: r[idx('timestamp_ns')] ?? '',
    errorCode: Number(r[idx('line')] ?? 0),
    freq: 1,
    level: (r[idx('level')] ?? 'error') as ErrorEntry['level'],
    latency: '',
    service: r[idx('service_name')] ?? '',
    message: r[idx('message')] ?? '',
    stackTrace: '',
  }))
}

function parseCharts(_rows: string[][], _columns: string[]): ErrorCharts {
  return MOCK_CHARTS
}

export function useErrors(): ErrorsPageProps {
  const { connected } = useAgent()

  const query = useQuery({
    queryKey: ['errors'],
    queryFn: async () => {
      if (!connected) {
        return { errors: MOCK_ERRORS, charts: MOCK_CHARTS, isWaiting: true }
      }
      const result = await postQuery(
        "SELECT event_id, timestamp_ns, level, service_name, message FROM logs WHERE level IN ('error','critical','fatal') ORDER BY timestamp_ns DESC LIMIT 1000",
      )
      if (!result) {
        return { errors: MOCK_ERRORS, charts: MOCK_CHARTS, isWaiting: true }
      }
      return {
        errors: rowsToErrors(result.rows, result.columns),
        charts: parseCharts(result.rows, result.columns),
        isWaiting: false,
      }
    },
    placeholderData: { errors: MOCK_ERRORS, charts: MOCK_CHARTS, isWaiting: true },
    enabled: connected,
  })

  const data = query.data ?? { errors: MOCK_ERRORS, charts: MOCK_CHARTS, isWaiting: true }

  return {
    errors: data.errors,
    totalErrors: data.errors.length,
    totalRows: data.errors.length,
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
  }
}