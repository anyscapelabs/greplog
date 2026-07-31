import { useState, useMemo } from 'react'
import { useServices, useFilterState, useRefreshControl } from '../hooks/index.ts'
import { useAgent } from '../context/AgentContext.tsx'
import ServicesDrawer from '../components/ServicesDrawer.tsx'
import PageHeader from '../components/PageHeader.tsx'
import ServiceCard from '../components/ServiceCard.tsx'
import ServicesFilterSidebar from '../components/ServicesFilterSidebar.tsx'
import ServicesTable from '../components/ServicesTable.tsx'
import WaitingOverlay, { SDK_SETUP_TERMINAL } from '../components/WaitingOverlay.tsx'
import AnalyticsChartPanel from '../components/AnalyticsChartPanel.tsx'
import RequestsByServiceChart from '../components/RequestsByServiceChart.tsx'
import ErrorRateByServiceChart from '../components/ErrorRateByServiceChart.tsx'
import AvgLatencyByServiceChart from '../components/AvgLatencyByServiceChart.tsx'
export default function Services() {
  const { connected } = useAgent()

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
    countRateOptions,
    latencyOptions,
    refetch,
  } = useServices(filters.timeRange)

  const {
    isLive,
    toggleLive,
    manualRefresh,
    autoRefresh,
    setAutoRefresh,
  } = useRefreshControl(refetch)

  const [filterOpen, setFilterOpen] = useState(true)
  const [drawerService, setDrawerService] = useState<any>(null)
  const [requestsMetric, setRequestsMetric] = useState('count')
  const [errorRateMetric, setErrorRateMetric] = useState('rate')
  const [latencyMetric, setLatencyMetric] = useState('avg')

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

  const healthServiceMap: Record<string, string[]> = {
    healthy: ['api', 'web'],
    degraded: ['db'],
  }

  const filteredServices = useMemo(() => {
    const checkedIds = Object.entries(filters.checked)
      .filter(([, v]) => v)
      .map(([k]) => k)
    const fromHealth = checkedIds
      .filter((id) => ['healthy', 'degraded'].includes(id))
      .flatMap((id) => healthServiceMap[id] || [])
    const allChecked = [...new Set([...filters.services, ...fromHealth])]
    if (allChecked.length === 0) return undefined
    return allChecked
  }, [filters.services, filters.checked])

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
          <WaitingOverlay
            visible={!connected}
            message="Run the Greplog agent and configure an SDK to start collecting services data"
            terminal={SDK_SETUP_TERMINAL}
          />
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
              <RequestsByServiceChart metric={requestsMetric} />
            </AnalyticsChartPanel>
            <AnalyticsChartPanel
              title="Error Rate by Service"
              dropdownItems={countRateOptions}
              dropdownValue={errorRateMetric}
              onDropdownChange={setErrorRateMetric}
              height="h-64"
            >
              <ErrorRateByServiceChart metric={errorRateMetric} />
            </AnalyticsChartPanel>
            <AnalyticsChartPanel
              title="Avg Latency by Service"
              dropdownItems={latencyOptions}
              dropdownValue={latencyMetric}
              onDropdownChange={setLatencyMetric}
              height="h-64"
            >
              <AvgLatencyByServiceChart metric={latencyMetric} />
            </AnalyticsChartPanel>
          </div>
          <ServicesTable data={services} totalRows={totalRows} querySeconds={querySeconds} filteredServices={filteredServices} onView={setDrawerService} />
        </div>
      </div>
      <ServicesDrawer open={!!drawerService} onClose={() => setDrawerService(null)} service={drawerService} />
    </div>
  )
}
