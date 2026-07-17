import { useState } from 'react'
import { LuRefreshCw, LuCircleDot, LuPanelLeftClose, LuPanelLeftOpen, LuServer, LuFilter, LuX } from 'react-icons/lu'
import FilterSidebar from '../components/FilterSidebar.tsx'
import LogVolumeChart from '../components/LogVolumeChart.tsx'
import ErrorsChart from '../components/ErrorsChart.tsx'
import StatusCodesChart from '../components/StatusCodesChart.tsx'
import LogsTable from '../components/LogsTable.tsx'
import Dropdown from '../components/Dropdown.tsx'

const timeRanges = ['Last 15 min', 'Last 1 hour', 'Last 6 hours', 'Last 24 hours', 'Last 7 days', 'Custom']

const chartMetrics = [
  { label: 'Count', value: 'count' },
  { label: 'Rate', value: 'rate' },
]

const requestsGroupBy = [
  { label: 'nothing', value: 'nothing' },
  { label: 'service', value: 'service' },
  { label: 'level', value: 'level' },
]

const errorsGroupBy = [
  { label: 'nothing', value: 'nothing' },
  { label: 'service', value: 'service' },
  { label: 'level', value: 'level' },
  { label: 'status_code', value: 'status_code' },
]

const statusCodesGroupBy = [
  { label: 'nothing', value: 'nothing' },
  { label: 'service', value: 'service' },
  { label: 'level', value: 'level' },
]

export default function Logs() {
  const [spinning, setSpinning] = useState(false)
  const [live, setLive] = useState(false)
  const [timeRange, setTimeRange] = useState('Last 15 min')
  const [filterOpen, setFilterOpen] = useState(true)
  const [autoRefresh, setAutoRefresh] = useState('Off')
  const services = ['All Services', 'web', 'api', 'db', 'worker']
  const [service, setService] = useState('All Services')
  const [query, setQuery] = useState('')
  const [chips, setChips] = useState<string[]>([])
  const [chart1Metric, setChart1Metric] = useState('count')
  const [chart1Group, setChart1Group] = useState('nothing')
  const [chart2Metric, setChart2Metric] = useState('count')
  const [chart2Group, setChart2Group] = useState('nothing')
  const [chart3Metric, setChart3Metric] = useState('count')
  const [chart3Group, setChart3Group] = useState('nothing')

  function handleQueryKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === 'Enter' && query.trim()) {
      setChips((prev) => [...prev, query.trim()])
      setQuery('')
    }
  }

  function removeChip(chip: string) {
    setChips((prev) => prev.filter((c) => c !== chip))
  }

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
          <span className="text-text-primary">Logs</span>
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
        {filterOpen && <FilterSidebar />}
        <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
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
            <div className="h-5 w-px mx-2" style={{ backgroundColor: 'var(--border-primary)' }} />
            <Dropdown
              trigger={<><LuServer className="size-3.5" style={{ color: 'var(--text-secondary)' }} /><span>{service}</span></>}
              items={services.map((s) => ({ label: s, value: s }))}
              value={service}
              onChange={setService}
              minWidth="min-w-32"
            />
            <div className="h-5 w-px mx-2" style={{ backgroundColor: 'var(--border-primary)' }} />
            <div className="flex-1 flex items-center gap-1.5 px-3 overflow-hidden">
              <LuFilter className="size-3.5 shrink-0" style={{ color: 'var(--text-secondary)' }} />
              <div className="flex items-center gap-1 flex-1 overflow-x-auto">
                {chips.map((chip) => (
                  <span
                    key={chip}
                    className="flex items-center gap-1 px-2 py-0.5 text-xs text-text-primary bg-[var(--bg-primary)] rounded-full whitespace-nowrap shrink-0"
                  >
                    {chip}
                    <button className="size-3.5 flex items-center justify-center rounded-full hover:bg-[var(--hover-bg-strong)] transition-colors" onClick={() => removeChip(chip)}>
                      <LuX className="size-2.5" />
                    </button>
                  </span>
                ))}
                <input
                  type="text"
                  placeholder={chips.length === 0 ? 'Search queries...' : ''}
                  className="flex-1 text-sm bg-transparent outline-none min-w-[120px]"
                  style={{ color: 'var(--text-primary)' }}
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  onKeyDown={handleQueryKeyDown}
                />
              </div>
            </div>
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
          <div className="flex gap-1.5 px-2 pt-2">
            <div className="flex-1 h-64 rounded border flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
                <span className="text-sm font-semibold text-text-primary">Total Requests</span>
                <div className="flex items-center gap-2">
                  <Dropdown
                    trigger={<span className="text-xs text-text-secondary">{chartMetrics.find(m => m.value === chart1Metric)?.label}</span>}
                    items={chartMetrics}
                    value={chart1Metric}
                    onChange={setChart1Metric}
                    minWidth="min-w-20"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                  <Dropdown
                    trigger={<span className="text-xs text-text-secondary">Grouped by {requestsGroupBy.find(g => g.value === chart1Group)?.label}</span>}
                    items={requestsGroupBy}
                    value={chart1Group}
                    onChange={setChart1Group}
                    minWidth="min-w-36"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                </div>
              </div>
              <div className="flex-1 p-1">
                <LogVolumeChart />
              </div>
            </div>
            <div className="flex-1 h-64 rounded border flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
                <span className="text-sm font-semibold text-text-primary">Errors</span>
                <div className="flex items-center gap-2">
                  <Dropdown
                    trigger={<span className="text-xs text-text-secondary">{chartMetrics.find(m => m.value === chart2Metric)?.label}</span>}
                    items={chartMetrics}
                    value={chart2Metric}
                    onChange={setChart2Metric}
                    minWidth="min-w-20"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                  <Dropdown
                    trigger={<span className="text-xs text-text-secondary">Grouped by {errorsGroupBy.find(g => g.value === chart2Group)?.label}</span>}
                    items={errorsGroupBy}
                    value={chart2Group}
                    onChange={setChart2Group}
                    minWidth="min-w-36"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                </div>
              </div>
              <div className="flex-1 p-1">
                <ErrorsChart />
              </div>
            </div>
            <div className="flex-1 h-64 rounded border flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
                <span className="text-sm font-semibold text-text-primary">Status Codes</span>
                <div className="flex items-center gap-2">
                  <Dropdown
                    trigger={<span className="text-xs text-text-secondary">{chartMetrics.find(m => m.value === chart3Metric)?.label}</span>}
                    items={chartMetrics}
                    value={chart3Metric}
                    onChange={setChart3Metric}
                    minWidth="min-w-20"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                  <Dropdown
                    trigger={<span className="text-xs text-text-secondary">Grouped by {statusCodesGroupBy.find(g => g.value === chart3Group)?.label}</span>}
                    items={statusCodesGroupBy}
                    value={chart3Group}
                    onChange={setChart3Group}
                    minWidth="min-w-36"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                </div>
              </div>
              <div className="flex-1 p-1">
                <StatusCodesChart />
              </div>
            </div>
          </div>
          <LogsTable />
        </div>
      </div>
    </div>
  )
}
