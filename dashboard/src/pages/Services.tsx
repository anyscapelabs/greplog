import { useState, useMemo } from 'react'
import ServicesDrawer from '../components/ServicesDrawer.tsx'
import { LuRefreshCw, LuCircleDot, LuPanelLeftClose, LuPanelLeftOpen } from 'react-icons/lu'
import ServiceCard from '../components/ServiceCard.tsx'
import ServicesFilterSidebar from '../components/ServicesFilterSidebar.tsx'
import ServicesTable from '../components/ServicesTable.tsx'
import AnalyticsChartPanel from '../components/AnalyticsChartPanel.tsx'
import RequestsByServiceChart from '../components/RequestsByServiceChart.tsx'
import ErrorRateByServiceChart from '../components/ErrorRateByServiceChart.tsx'
import AvgLatencyByServiceChart from '../components/AvgLatencyByServiceChart.tsx'
import Dropdown from '../components/Dropdown.tsx'

const timeRanges = ['Last 15 min', 'Last 1 hour', 'Last 6 hours', 'Last 24 hours', 'Last 7 days', 'Custom']

const countRateOptions = [
  { label: 'Count', value: 'count' },
  { label: 'Rate', value: 'rate' },
]

const latencyOptions = [
  { label: 'Avg', value: 'avg' },
  { label: 'P50', value: 'p50' },
  { label: 'P95', value: 'p95' },
  { label: 'P99', value: 'p99' },
]

const generateData = (base: number, variance: number, min: number = 0) => {
  let current = base
  return Array.from({ length: 60 }, () => {
    current += (Math.random() - 0.5) * variance
    if (current < min) current = min
    return current
  })
}

const serviceCards = [
  { name: 'api', requests: '2,500 req/s', data: generateData(2500, 500) },
  { name: 'web', requests: '1,850 req/s', data: generateData(1850, 400) },
  { name: 'db', requests: '940 req/s', data: generateData(940, 200) },
  { name: 'worker', requests: '250 req/s', data: generateData(250, 80) },
]

export default function Services() {
  const [spinning, setSpinning] = useState(false)
  const [live, setLive] = useState(false)
  const [timeRange, setTimeRange] = useState('Last 15 min')
  const [filterOpen, setFilterOpen] = useState(true)
  const [drawerService, setDrawerService] = useState<any>(null)
  const [autoRefresh, setAutoRefresh] = useState('Off')
  const [requestsMetric, setRequestsMetric] = useState('count')
  const [errorRateMetric, setErrorRateMetric] = useState('rate')
  const [latencyMetric, setLatencyMetric] = useState('avg')
  const [checked, setChecked] = useState<Record<string, boolean>>({})

  function handleCheck(id: string) {
    setChecked((prev) => ({ ...prev, [id]: !prev[id] }))
  }

  const filteredServices = useMemo(() => {
    const checkedServiceIds = Object.entries(checked)
      .filter(([, v]) => v)
      .map(([k]) => k)
    const healthServiceMap: Record<string, string[]> = {
      healthy: ['api', 'web'],
      degraded: ['db'],
      down: ['worker'],
    }
    const fromHealth = checkedServiceIds
      .filter(id => ['healthy', 'degraded', 'down'].includes(id))
      .flatMap(id => healthServiceMap[id] || [])
    const fromServices = checkedServiceIds.filter(id => !['healthy', 'degraded', 'down'].includes(id))
    const allChecked = [...new Set([...fromHealth, ...fromServices])]
    if (allChecked.length === 0) return undefined
    return allChecked
  }, [checked])

  return (
    <div className="flex flex-col h-full">
      <div
        className="flex items-center px-4 h-12 shrink-0 border-b gap-3"
        style={{
          backgroundColor: 'var(--bg-secondary)',
          borderColor: 'var(--border-primary)',
        }}
      >
        <span className="text-2xl font-semibold flex items-center gap-2">
          <span style={{ color: 'var(--text-secondary)' }}>Grep</span>
          <span className="text-text-primary">Services</span>
        </span>
        <div className="ml-auto flex items-center gap-2">
          <button
            className="flex items-center gap-1.5 px-2 py-1 text-sm text-text-primary hover:bg-[var(--hover-bg)] transition-colors"
            style={{
              borderColor: 'var(--border-primary)',
              borderWidth: 1,
            }}
            onClick={() => {
              setSpinning(true)
              setTimeout(() => setSpinning(false), 600)
            }}
          >
            <LuRefreshCw className={`size-3.5 ${spinning ? 'animate-spin' : ''}`} />
            Refresh
          </button>
          <button
            className={`flex items-center gap-1.5 px-2 py-1 text-sm transition-colors ${
              live ? 'text-white bg-success border-success' : 'text-text-primary hover:bg-[var(--hover-bg)]'
            }`}
            style={{
              borderColor: live ? undefined : 'var(--border-primary)',
              borderWidth: 1,
            }}
            onClick={() => setLive(!live)}
          >
            <LuCircleDot className="size-3.5" />
            Live
          </button>
        </div>
      </div>
      <div className="flex flex-1 min-h-0">
        {filterOpen && <ServicesFilterSidebar checked={checked} onCheck={handleCheck} />}
        <div className="flex-1 flex flex-col min-w-0">
          <div
            className="flex items-center h-10 border-b shrink-0"
            style={{
              backgroundColor: 'var(--bg-secondary)',
              borderColor: 'var(--border-primary)',
            }}
          >
            <button
              className="flex items-center justify-center p-1.5 text-text-primary hover:bg-[var(--hover-bg)] transition-colors ml-1"
              onClick={() => setFilterOpen(!filterOpen)}
              title={filterOpen ? 'Close filters' : 'Open filters'}
            >
              {filterOpen ? <LuPanelLeftClose className="size-4" /> : <LuPanelLeftOpen className="size-4" />}
            </button>
            <div className="ml-auto flex items-center gap-2 pr-4">
              <Dropdown
                trigger={<><span className="text-text-primary text-sm">Auto refresh</span>{autoRefresh !== 'Off' && <span className="flex items-center justify-center px-1.5 py-0.5 text-xs text-text-primary bg-[var(--bg-primary)] rounded">{autoRefresh}</span>}</>}
                items={['Off', '10s', '30s', '1m', '5m'].map((opt) => ({ label: opt, value: opt }))}
                value={autoRefresh}
                onChange={setAutoRefresh}
                align="right"
                minWidth="min-w-16"
                hasBorder
              />
              <Dropdown
                trigger={<span>{timeRange}</span>}
                items={timeRanges.map((r) => ({ label: r, value: r }))}
                value={timeRange}
                onChange={setTimeRange}
                align="right"
                minWidth="min-w-40"
                hasBorder
              />
            </div>
          </div>
          <div className="flex gap-1.5 px-2 pt-2 pb-1.5 shrink-0">
            {serviceCards.map((card) => (
              <ServiceCard key={card.name} {...card} />
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
          <ServicesTable filteredServices={filteredServices} onView={setDrawerService} />
        </div>
      </div>
      <ServicesDrawer open={!!drawerService} onClose={() => setDrawerService(null)} service={drawerService} />
    </div>
  )
}
