import { useQuery } from '@tanstack/react-query'
import type { ServicesPageProps, ServiceEntry, ServiceCharts } from '../types/index.ts'
import { postQuery } from './api.ts'
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
import { useAgent } from '../context/AgentContext.tsx'

const MOCK_SERVICES = placeholderServiceEntries
const MOCK_CARDS = placeholderServiceCards
const MOCK_CHARTS = placeholderServiceCharts

function rowsToServices(rows: string[][], columns: string[]): ServiceEntry[] {
  const idx = (name: string) => columns.indexOf(name)
  return rows.map((r) => ({
    id: r[idx('service_name')] ?? '',
    name: r[idx('service_name')] ?? '',
    status: 'healthy' as ServiceEntry['status'],
    uptime: '',
    requests: '0',
    errorRate: '0%',
    avgLatency: '0ms',
    p95: '0ms',
    p99: '0ms',
    lastSeen: r[idx('max_ts')] ?? '',
  }))
}

function parseCharts(_rows: string[][], _columns: string[]): ServiceCharts {
  return MOCK_CHARTS
}

export function useServices(): ServicesPageProps {
  const { connected } = useAgent()

  const query = useQuery({
    queryKey: ['services'],
    queryFn: async () => {
      if (!connected) {
        return { services: MOCK_SERVICES, serviceCards: MOCK_CARDS, charts: MOCK_CHARTS, isWaiting: true }
      }
      const result = await postQuery(
        'SELECT service_name, max(timestamp_ns) AS max_ts FROM logs GROUP BY service_name ORDER BY max_ts DESC',
      )
      if (!result) {
        return { services: MOCK_SERVICES, serviceCards: MOCK_CARDS, charts: MOCK_CHARTS, isWaiting: true }
      }
      return {
        services: rowsToServices(result.rows, result.columns),
        serviceCards: MOCK_CARDS,
        charts: parseCharts(result.rows, result.columns),
        isWaiting: false,
      }
    },
    placeholderData: { services: MOCK_SERVICES, serviceCards: MOCK_CARDS, charts: MOCK_CHARTS, isWaiting: true },
    enabled: connected,
  })

  const data = query.data ?? { services: MOCK_SERVICES, serviceCards: MOCK_CARDS, charts: MOCK_CHARTS, isWaiting: true }

  return {
    services: data.services,
    totalRows: data.services.length,
    querySeconds: 0,
    filterSections: placeholderServiceFilterSections,
    serviceCards: data.serviceCards,
    charts: data.charts,
    isWaiting: data.isWaiting,
    timeRanges: placeholderTimeRanges,
    autoRefreshOptions: placeholderAutoRefreshOptions,
    countRateOptions: placeholderCountRateOptions,
    latencyOptions: placeholderLatencyOptions,
    onViewService: undefined,
  }
}