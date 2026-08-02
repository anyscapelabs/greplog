import { useState, useMemo } from 'react'
import { useServices, useFilterState, useRefreshControl } from '../hooks/index.ts'
import ServicesDrawer from '../components/ServicesDrawer.tsx'
import PageHeader from '../components/PageHeader.tsx'
import ServiceCard from '../components/ServiceCard.tsx'
import ServicesFilterSidebar from '../components/ServicesFilterSidebar.tsx'
import ServicesTable from '../components/ServicesTable.tsx'
import AnalyticsChartPanel from '../components/AnalyticsChartPanel.tsx'
import RequestsByServiceChart from '../components/RequestsByServiceChart.tsx'
import ErrorRateByServiceChart from '../components/ErrorRateByServiceChart.tsx'
import AvgLatencyByServiceChart from '../components/AvgLatencyByServiceChart.tsx'
import type { ServiceEntry } from '../types/index.ts'
export default function Services() {

  const {
    filters,
    toggleService,
    setTimeRange,
    toggleChecked,
  } = useFilterState()

  const {
    services,
    totalRows,
    querySeconds,
    filterSections,
    serviceCards,
    charts,
    countRateOptions,
    latencyOptions,
    refetch,
    manualRefetch,
  } = useServices(filters.timeRange)

  const {
    isLive,
    toggleLive,
    manualRefresh,
    autoRefresh,
    setAutoRefresh,
  } = useRefreshControl(refetch, { manualRefetch })

  const [filterOpen, setFilterOpen] = useState(true)
  const [drawerService, setDrawerService] = useState<ServiceEntry | null>(null)
  const [requestsMetric, setRequestsMetric] = useState('count')
  const [errorRateMetric, setErrorRateMetric] = useState('rate')
  const [latencyMetric, setLatencyMetric] = useState('avg')

  const requestsData = useMemo(
    () => charts.requests.map((r) => ({ label: r.service, value: requestsMetric === 'rate' ? r.rate : r.count })),
    [charts.requests, requestsMetric],
  )

  const errorRateData = useMemo(
    () => charts.errorRates.map((r) => ({ label: r.service, value: errorRateMetric === 'count' ? r.count : r.rate })),
    [charts.errorRates, errorRateMetric],
  )

  const latencyData = useMemo(
    () => charts.latencies.map((r) => ({ label: r.service, value: r[latencyMetric as 'avg' | 'p50' | 'p95' | 'p99'] ?? r.avg })),
    [charts.latencies, latencyMetric],
  )

  function handleCheck(id: string) {
    const section = filterSections.find((s) => s.items.some((i) => i.id === id))
    if (section?.id === 'service_name') {
      toggleService(id)
    } else {
      toggleChecked(id)
    }
  }

  const checked = useMemo(() => {
    const merged: Record<string, boolean> = { ...filters.checked }
    for (const s of filters.services) {
      merged[s] = true
    }
    return merged
  }, [filters.checked, filters.services])

  const filteredServices = useMemo(() => {
    const healthServiceMap: Record<string, string[]> = {}
    for (const svc of services) {
      if (!healthServiceMap[svc.health]) healthServiceMap[svc.health] = []
      healthServiceMap[svc.health].push(svc.name)
    }
    const checkedIds = Object.entries(filters.checked)
      .filter(([, v]) => v)
      .map(([k]) => k)
    const fromHealth = checkedIds
      .filter((id) => healthServiceMap[id])
      .flatMap((id) => healthServiceMap[id] ?? [])
    const allChecked = [...new Set([...filters.services, ...fromHealth])]
    if (allChecked.length === 0) return undefined
    return allChecked
  }, [filters.services, filters.checked, services])

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="Services"
        showLive
        isLive={isLive}
        onLiveChange={toggleLive}
        onRefresh={manualRefresh}
        showFilterToggle
        timeRange={filters.timeRange}
        onTimeRangeChange={setTimeRange}
        autoRefresh={autoRefresh}
        onAutoRefreshChange={setAutoRefresh}
        filterOpen={filterOpen}
        onFilterToggle={() => setFilterOpen(!filterOpen)}
      />
      <div className="flex flex-1 min-h-0 relative">
        {filterOpen && <ServicesFilterSidebar checked={checked} onCheck={handleCheck} sections={filterSections} />}
        <div className="flex-1 flex flex-col min-w-0">
          <div className="flex gap-1.5 px-2 pt-2 pb-1.5 shrink-0">
            {serviceCards.map((card) => (
              <ServiceCard key={card.name} name={card.name} requests={card.label} data={card.sparkline} />
            ))}
          </div>
          <div className="grid grid-cols-3 gap-1.5 px-2 pt-1 pb-1.5 shrink-0">
            <AnalyticsChartPanel
              title="Requests by Service"
              dropdownItems={countRateOptions}
              dropdownValue={requestsMetric}
              onDropdownChange={setRequestsMetric}
              height="h-64"
            >
              <RequestsByServiceChart metric={requestsMetric} data={requestsData} />
            </AnalyticsChartPanel>
            <AnalyticsChartPanel
              title="Error Rate by Service"
              dropdownItems={countRateOptions}
              dropdownValue={errorRateMetric}
              onDropdownChange={setErrorRateMetric}
              height="h-64"
            >
              <ErrorRateByServiceChart metric={errorRateMetric} data={errorRateData} />
            </AnalyticsChartPanel>
            <AnalyticsChartPanel
              title="Avg Latency by Service"
              dropdownItems={latencyOptions}
              dropdownValue={latencyMetric}
              onDropdownChange={setLatencyMetric}
              height="h-64"
            >
              <AvgLatencyByServiceChart metric={latencyMetric} data={latencyData} />
            </AnalyticsChartPanel>
          </div>
          <ServicesTable data={services} totalRows={totalRows} querySeconds={querySeconds} filteredServices={filteredServices} onView={setDrawerService} />
        </div>
      </div>
      <ServicesDrawer open={!!drawerService} onClose={() => setDrawerService(null)} service={drawerService} />
    </div>
  )
}
