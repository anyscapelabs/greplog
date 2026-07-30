import { useQuery } from '@tanstack/react-query'
import type { LogsPageProps, LogEntry, LogCharts } from '../types/index.ts'
import { postQuery } from './api.ts'
import {
  placeholderLogs,
  placeholderLogFilterSections,
  placeholderLogCharts,
  placeholderTimeRanges,
  placeholderServices,
  placeholderAutoRefreshOptions,
  placeholderChartMetrics,
  placeholderRequestsGroupBy,
  placeholderErrorsGroupBy,
  placeholderStatusCodesGroupBy,
} from './placeholder-data.ts'
import { useAgent } from '../context/AgentContext.tsx'

const MOCK_LOGS = placeholderLogs(50000)
const MOCK_CHARTS = placeholderLogCharts()

function rowsToLogs(rows: string[][], columns: string[]): LogEntry[] {
  const idx = (name: string) => columns.indexOf(name)
  return rows.map((r) => ({
    id: r[idx('event_id')] ?? '',
    timestamp: r[idx('timestamp_ns')] ?? '',
    level: (r[idx('level')] ?? 'info') as LogEntry['level'],
    service: r[idx('service_name')] ?? '',
    statusCode: Number(r[idx('line')] ?? 0),
    message: r[idx('message')] ?? '',
    response: '',
    logger: r[idx('logger_name')] ?? '',
    correlationId: r[idx('correlation_id')] ?? '',
    file: r[idx('file')] ?? '',
  }))
}

function parseCharts(_rows: string[][], _columns: string[]): LogCharts {
  return MOCK_CHARTS
}

export function useLogs(): LogsPageProps {
  const { connected } = useAgent()

  const query = useQuery({
    queryKey: ['logs'],
    queryFn: async () => {
      if (!connected) {
        return { logs: MOCK_LOGS, charts: MOCK_CHARTS, isWaiting: true }
      }
      const result = await postQuery(
        'SELECT event_id, timestamp_ns, level, service_name, message, logger_name, file, line, correlation_id FROM logs ORDER BY timestamp_ns DESC LIMIT 1000',
      )
      if (!result) {
        return { logs: MOCK_LOGS, charts: MOCK_CHARTS, isWaiting: true }
      }
      return {
        logs: rowsToLogs(result.rows, result.columns),
        charts: parseCharts(result.rows, result.columns),
        isWaiting: false,
      }
    },
    placeholderData: { logs: MOCK_LOGS, charts: MOCK_CHARTS, isWaiting: true },
    enabled: connected,
  })

  const data = query.data ?? { logs: MOCK_LOGS, charts: MOCK_CHARTS, isWaiting: true }

  return {
    logs: data.logs,
    totalLogs: data.logs.length,
    totalRows: data.logs.length,
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
  }
}