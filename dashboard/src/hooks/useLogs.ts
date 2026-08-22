import { useQuery } from '@tanstack/react-query'
import { logApi, type QueryFilters } from '../api/logs'
import { RANGE_SECONDS, binIntervalSeconds } from '../components/logs/Timeline'
import type { TimeRange } from '../components/Header'

function validateFilters(filters: QueryFilters): void {
  if (!filters) throw new Error('Query filters are required')

  if (!Number.isFinite(filters.timeRangeSecs) || filters.timeRangeSecs <= 0) {
    throw new Error('filters.timeRangeSecs must be a positive number')
  }
}

function validateRange(range: TimeRange): number {
  const rangeSecs = RANGE_SECONDS[range]
  if (!rangeSecs) throw new Error(`Invalid time range: ${range}`)

  return rangeSecs
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message

  if (typeof error === 'string') return error

  return 'Unknown error'
}

export function useLogExplorer(filters: QueryFilters | null, range: TimeRange) {
  const logsQuery = useQuery({
    queryKey: ['logs', filters],
    queryFn: () => {
      if (!filters) throw new Error('Query filters are required')

      return logApi.fetchLogs(filters)
    },
    enabled: filters !== null,
  })

  const histogramQuery = useQuery({
    queryKey: ['histogram', filters, range],
    queryFn: () => {
      if (!filters) throw new Error('Query filters are required')
      validateRange(range)

      return logApi.fetchHistogram(filters, binIntervalSeconds(range))
    },
    enabled: filters !== null,
  })

  const facetsQuery = useQuery({
    queryKey: ['facets', filters],
    queryFn: () => {
      if (!filters) throw new Error('Query filters are required')

      return logApi.fetchFacets(filters)
    },
    enabled: filters !== null,
  })

  const isLoading = logsQuery.isLoading || histogramQuery.isLoading || facetsQuery.isLoading
  const isError = logsQuery.isError || histogramQuery.isError || facetsQuery.isError
  const error = logsQuery.error ?? histogramQuery.error ?? facetsQuery.error
  const errorMessage = isError ? getErrorMessage(error) : undefined

  return {
    logs: logsQuery.data,
    histogram: histogramQuery.data,
    facets: facetsQuery.data,
    isLoading,
    isError,
    error,
    errorMessage,
    refetchLogs: logsQuery.refetch,
  }
}

export function useIngestion(range: TimeRange) {
  const ingestionQuery = useQuery({
    queryKey: ['ingestion', range],
    queryFn: () => logApi.fetchIngestion(validateRange(range), binIntervalSeconds(range)),
  })

  return {
    data: ingestionQuery.data,
    isLoading: ingestionQuery.isLoading,
    isError: ingestionQuery.isError,
    error: ingestionQuery.error,
    errorMessage: ingestionQuery.isError ? getErrorMessage(ingestionQuery.error) : undefined,
  }
}

export function useSeverityBreakdown(filters: QueryFilters, range: TimeRange) {
  const severityQuery = useQuery({
    queryKey: ['severity-breakdown', filters, range],
    queryFn: () => {
      validateFilters(filters)
      validateRange(range)

      return logApi.fetchSeverityBreakdown(filters, binIntervalSeconds(range))
    },
  })

  return {
    data: severityQuery.data,
    isLoading: severityQuery.isLoading,
    isError: severityQuery.isError,
    error: severityQuery.error,
    errorMessage: severityQuery.isError ? getErrorMessage(severityQuery.error) : undefined,
  }
}

export function useIngestionByService(filters: QueryFilters, range: TimeRange) {
  const ingestionByServiceQuery = useQuery({
    queryKey: ['ingestion-by-service', filters, range],
    queryFn: () => {
      validateFilters(filters)
      validateRange(range)

      return logApi.fetchIngestionByService(filters, binIntervalSeconds(range))
    },
  })

  return {
    data: ingestionByServiceQuery.data,
    isLoading: ingestionByServiceQuery.isLoading,
    isError: ingestionByServiceQuery.isError,
    error: ingestionByServiceQuery.error,
    errorMessage: ingestionByServiceQuery.isError ? getErrorMessage(ingestionByServiceQuery.error) : undefined,
  }
}

export function useServiceTable(filters: QueryFilters) {
  const serviceTableQuery = useQuery({
    queryKey: ['service-table', filters],
    queryFn: () => {
      validateFilters(filters)

      return logApi.fetchServiceTable(filters)
    },
  })

  return {
    data: serviceTableQuery.data,
    isLoading: serviceTableQuery.isLoading,
    isError: serviceTableQuery.isError,
    error: serviceTableQuery.error,
    errorMessage: serviceTableQuery.isError ? getErrorMessage(serviceTableQuery.error) : undefined,
  }
}

export interface ScalarMetric {
  value: number | null
  isLoading: boolean
  isError: boolean
  errorMessage?: string
}

export function useErrorRate(filters: QueryFilters): ScalarMetric {
  const errorRateQuery = useQuery({
    queryKey: ['error-rate', filters],
    queryFn: () => {
      validateFilters(filters)

      return logApi.fetchErrorRate(filters)
    },
  })

  const row = errorRateQuery.data?.[0] as { count?: unknown; errors?: unknown } | undefined
  const total = Number(row?.count ?? 0)
  const errors = Number(row?.errors ?? 0)
  const hasRows = (errorRateQuery.data?.length ?? 0) > 0 && total > 0

  return {
    value: hasRows ? (errors / total) * 100 : null,
    isLoading: errorRateQuery.isLoading,
    isError: errorRateQuery.isError,
    errorMessage: errorRateQuery.isError ? getErrorMessage(errorRateQuery.error) : undefined,
  }
}

export function useStorage() {
  const storageQuery = useQuery({
    queryKey: ['storage'],
    queryFn: () => logApi.fetchStorage(),
  })

  const bytes = storageQuery.data?.bytes
  return {
    valueGb: bytes === undefined ? null : bytes / 1e9,
    isLoading: storageQuery.isLoading,
    isError: storageQuery.isError,
    errorMessage: storageQuery.isError ? getErrorMessage(storageQuery.error) : undefined,
    stats: storageQuery.data
      ? { partitions: storageQuery.data.partitions, chunks: storageQuery.data.chunks }
      : undefined,
  }
}
