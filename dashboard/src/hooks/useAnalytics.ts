import { useQuery } from '@tanstack/react-query'
import type { AnalyticsPageProps } from '../types/index.ts'
import { postQuery, type QueryResult } from './api.ts'
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
import { useAgent } from '../context/AgentContext.tsx'

const PLACEHOLDER = {
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
  isWaiting: true,
}

function parseCountTimeseries(result: QueryResult): AnalyticsPageProps['ingestionTimeseries'] {
  const idx = (name: string) => result.columns.indexOf(name)
  const tsIdx = idx('date')
  const cntIdx = idx('cnt')
  if (tsIdx < 0 || cntIdx < 0) return placeholderIngestionTimeseries()
  return result.rows.map((r) => ({
    timestamp: r[tsIdx] ?? '',
    value: Number(r[cntIdx] ?? 0),
  }))
}

export function useAnalytics(): AnalyticsPageProps {
  const { connected } = useAgent()

  const query = useQuery({
    queryKey: ['analytics'],
    queryFn: async () => {
      if (!connected) return PLACEHOLDER
      const result = await postQuery(
        "SELECT date, count(*) AS cnt FROM logs GROUP BY date ORDER BY date",
      )
      if (!result) return PLACEHOLDER
      return {
        metrics: placeholderAnalyticsMetrics,
        ingestionTimeseries: parseCountTimeseries(result),
        errorRateTimeseries: placeholderErrorRateTimeseries(),
        latencyData: placeholderLatencyData,
        serviceHealthData: placeholderServiceHealth,
        statusCodeDistribution: placeholderStatusCodeDistribution,
        noisyServices: placeholderNoisyServices,
        severityDistribution: placeholderSeverityDistribution,
        systemMetrics: placeholderSystemMetrics,
        avgResponseTimes: placeholderAvgResponseTimes,
        isWaiting: false,
      }
    },
    placeholderData: PLACEHOLDER,
    enabled: connected,
  })

  const data = query.data ?? PLACEHOLDER

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