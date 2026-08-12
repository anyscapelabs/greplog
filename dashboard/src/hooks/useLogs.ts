import { useQuery } from '@tanstack/react-query'
import { logApi, type QueryFilters } from '../api/logs'

export function useLogExplorer(filters: QueryFilters) {
  // The queryKey ensures that whenever a filter changes, the data is
  // automatically refetched.
  const logsQuery = useQuery({
    queryKey: ['logs', filters],
    queryFn: () => logApi.fetchLogs(filters),
  })

  const histogramQuery = useQuery({
    queryKey: ['histogram', filters],
    queryFn: () => logApi.fetchHistogram(filters),
  })

  const facetsQuery = useQuery({
    queryKey: ['facets', filters],
    queryFn: () => logApi.fetchFacets(filters),
  })

  return {
    logs: logsQuery.data,
    histogram: histogramQuery.data,
    facets: facetsQuery.data,
    isLoading:
      logsQuery.isLoading || histogramQuery.isLoading || facetsQuery.isLoading,
    isError: logsQuery.isError || histogramQuery.isError || facetsQuery.isError,
    refetchLogs: logsQuery.refetch,
  }
}