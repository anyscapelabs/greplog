import { useQuery } from '@tanstack/react-query'
import type { AnalyticsPageProps } from '../types/index.ts'
import {
  placeholderAnalyticsMetrics,
  placeholderIngestionTimeseries,
  placeholderErrorRateTimeseries,
  placeholderLatencyData,
  placeholderServiceHealth,
  placeholderStatusCodeDistribution,
  placeholderNoisyServices,
  placeholderSeverityDistribution,
  placeholderSystemMetrics,
  placeholderAvgResponseTimes,
  placeholderTimeRanges,
  placeholderServices,
  placeholderAutoRefreshOptions,
  placeholderAnalyticsIngestionOptions,
  placeholderAnalyticsRateCountOptions,
  placeholderAnalyticsLatencyOptions,
  placeholderSortOptions,
} from './placeholder-data.ts'

const defaultData = {
  metrics: placeholderAnalyticsMetrics,
  ingestionTimeseries: placeholderIngestionTimeseries(),
  errorRateTimeseries: placeholderErrorRateTimeseries(),
  latencyData: placeholderLatencyData,
  serviceHealthData: placeholderServiceHealth,
  statusCodeDistribution: placeholderStatusCodeDistribution,
  noisyServices: placeholderNoisyServices,
  severityDistribution: placeholderSeverityDistribution,
  systemMetrics: placeholderSystemMetrics,
  avgResponseTimes: placeholderAvgResponseTimes,
}

export function useAnalytics(): AnalyticsPageProps {
  const query = useQuery({
    queryKey: ['analytics'],
    queryFn: async () => {
      await new Promise((r) => setTimeout(r, 150))
      return defaultData
    },
    placeholderData: defaultData,
  })

  const data = query.data ?? defaultData

  return {
    ...data,
    timeRanges: placeholderTimeRanges,
    services: placeholderServices,
    autoRefreshOptions: placeholderAutoRefreshOptions,
    ingestionOptions: placeholderAnalyticsIngestionOptions,
    rateCountOptions: placeholderAnalyticsRateCountOptions,
    latencyOptions: placeholderAnalyticsLatencyOptions,
    sortOptions: placeholderSortOptions,
    ingestionMetric: 'sum',
    onIngestionMetricChange: () => {},
    errorRateMetric: 'rate',
    onErrorRateMetricChange: () => {},
    latencyView: 'p50_p90_p99',
    onLatencyViewChange: () => {},
    statusCodeMetric: 'rate',
    onStatusCodeMetricChange: () => {},
    noisySort: 'logs',
    onNoisySortChange: () => {},
  }
}