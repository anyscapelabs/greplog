import { useQuery } from '@tanstack/react-query'
import type { ServicesPageProps } from '../types/index.ts'
import {
  placeholderServiceEntries,
  placeholderServiceCards,
  placeholderServiceCharts,
  placeholderServiceFilterSections,
  placeholderTimeRanges,
  placeholderAutoRefreshOptions,
  placeholderCountRateOptions,
  placeholderLatencyOptions,
} from './placeholder-data.ts'

const defaultData = {
  services: placeholderServiceEntries,
  serviceCards: placeholderServiceCards,
  charts: placeholderServiceCharts,
}

export function useServices(): ServicesPageProps {
  const query = useQuery({
    queryKey: ['services'],
    queryFn: async () => {
      await new Promise((r) => setTimeout(r, 150))
      return defaultData
    },
    placeholderData: defaultData,
  })

  const data = query.data ?? defaultData

  return {
    services: data.services,
    totalRows: data.services.length,
    querySeconds: 0.12,
    filterSections: placeholderServiceFilterSections,
    serviceCards: data.serviceCards,
    charts: data.charts,
    timeRanges: placeholderTimeRanges,
    autoRefreshOptions: placeholderAutoRefreshOptions,
    countRateOptions: placeholderCountRateOptions,
    latencyOptions: placeholderLatencyOptions,
    onViewService: undefined,
  }
}