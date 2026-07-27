import { useQuery } from '@tanstack/react-query'
import type { ErrorsPageProps } from '../types/index.ts'
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

const defaultErrors = placeholderErrors(50000)

const defaultData = {
  errors: defaultErrors,
  charts: placeholderErrorCharts(),
  isWaiting: false,
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

export function useErrors(): ErrorsPageProps {
  const query = useQuery({
    queryKey: ['errors'],
    queryFn: async () => {
      await new Promise((r) => setTimeout(r, 200))
      return defaultData
    },
    placeholderData: defaultData,
  })

  const data = query.data ?? defaultData

  return {
    errors: data.errors,
    totalErrors: data.errors.length,
    totalRows: data.errors.length,
    querySeconds: 0.28,
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