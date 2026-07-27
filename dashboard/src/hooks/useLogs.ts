import { useQuery } from '@tanstack/react-query'
import type { LogsPageProps } from '../types/index.ts'
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

const defaultLogs = placeholderLogs(50000)

const defaultData = {
  logs: defaultLogs,
  charts: placeholderLogCharts(),
  isWaiting: false,
}

export function useLogs(): LogsPageProps {
  const query = useQuery({
    queryKey: ['logs'],
    queryFn: async () => {
      await new Promise((r) => setTimeout(r, 200))
      return defaultData
    },
    placeholderData: defaultData,
  })

  const data = query.data ?? defaultData

  return {
    logs: data.logs,
    totalLogs: data.logs.length,
    totalRows: data.logs.length,
    querySeconds: 0.32,
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