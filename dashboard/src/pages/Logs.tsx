import { useState } from 'react'
import { LuRefreshCw, LuCircleDot, LuChevronDown, LuPanelLeftClose, LuPanelLeftOpen, LuServer, LuFilter, LuX } from 'react-icons/lu'
import FilterSidebar from '../components/FilterSidebar.tsx'
import LogVolumeChart from '../components/LogVolumeChart.tsx'
import ErrorsChart from '../components/ErrorsChart.tsx'
import StatusCodesChart from '../components/StatusCodesChart.tsx'
import LogsTable from '../components/LogsTable.tsx'

const timeRanges = ['Last 15 min', 'Last 1 hour', 'Last 6 hours', 'Last 24 hours', 'Last 7 days', 'Custom']

export default function Logs() {
  const [spinning, setSpinning] = useState(false)
  const [live, setLive] = useState(false)
  const [timeRange, setTimeRange] = useState('Last 15 min')
  const [timeOpen, setTimeOpen] = useState(false)
  const [filterOpen, setFilterOpen] = useState(true)
  const [autoRefresh, setAutoRefresh] = useState('Off')
  const [refreshOpen, setRefreshOpen] = useState(false)
  const services = ['All Services', 'web', 'api', 'db', 'worker']
  const [service, setService] = useState('All Services')
  const [serviceOpen, setServiceOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [chips, setChips] = useState<string[]>([])

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
            className="flex items-center gap-1.5 px-2 py-1 text-sm text-text-primary hover:bg-gray-100 transition-colors"
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
              live ? 'text-white bg-success border-success' : 'text-text-primary hover:bg-gray-100'
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
              className="flex items-center justify-center p-1.5 text-text-primary hover:bg-gray-100 transition-colors ml-1"
              onClick={() => setFilterOpen(!filterOpen)}
              title={filterOpen ? 'Close filters' : 'Open filters'}
            >
              {filterOpen ? <LuPanelLeftClose className="size-4" /> : <LuPanelLeftOpen className="size-4" />}
            </button>
            <div className="h-5 w-px mx-2" style={{ backgroundColor: 'var(--border-primary)' }} />
            <div className="relative">
              <button
                className="flex items-center gap-1.5 px-2 py-1 text-sm text-text-primary hover:bg-gray-100 transition-colors"
                onClick={() => setServiceOpen(!serviceOpen)}
              >
                <LuServer className="size-3.5" style={{ color: 'var(--text-secondary)' }} />
                <span>{service}</span>
                <LuChevronDown className="size-3.5" style={{ color: 'var(--text-secondary)' }} />
              </button>
              {serviceOpen && (
                <div
                  className="absolute top-full left-0 mt-1 py-1 min-w-32 rounded border bg-white shadow-md z-50"
                  style={{ borderColor: 'var(--border-primary)' }}
                >
                  {services.map((s) => (
                    <button
                      key={s}
                      className={`w-full text-left px-3 py-1.5 text-sm transition-colors ${
                        s === service ? 'text-text-primary bg-gray-100 font-medium' : 'text-text-primary hover:bg-gray-50'
                      }`}
                      onClick={() => { setService(s); setServiceOpen(false) }}
                    >
                      {s}
                    </button>
                  ))}
                </div>
              )}
            </div>
            <div className="h-5 w-px mx-2" style={{ backgroundColor: 'var(--border-primary)' }} />
            <div className="flex-1 flex items-center gap-1.5 px-3 overflow-hidden">
              <LuFilter className="size-3.5 shrink-0" style={{ color: 'var(--text-secondary)' }} />
              <div className="flex items-center gap-1 flex-1 overflow-x-auto">
                {chips.map((chip) => (
                  <span
                    key={chip}
                    className="flex items-center gap-1 px-2 py-0.5 text-xs text-text-primary bg-gray-100 rounded-full whitespace-nowrap shrink-0"
                  >
                    {chip}
                    <button className="size-3.5 flex items-center justify-center rounded-full hover:bg-gray-200 transition-colors" onClick={() => removeChip(chip)}>
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
              <div className="relative">
                <button
                  className="flex items-center gap-1.5 px-2 py-1 text-sm hover:bg-gray-100 transition-colors"
                  style={{
                    borderColor: 'var(--border-primary)',
                    borderWidth: 1,
                  }}
                  onClick={() => setRefreshOpen(!refreshOpen)}
                >
                  <span className="text-text-primary text-sm">Auto refresh</span>
                  {autoRefresh !== 'Off' && (
                    <span className="flex items-center justify-center px-1.5 py-0.5 text-xs text-text-primary bg-gray-100 rounded">
                      {autoRefresh}
                    </span>
                  )}
                </button>
                {refreshOpen && (
                  <div
                    className="absolute top-full right-0 mt-1 py-1 min-w-16 rounded border bg-white shadow-md z-50"
                    style={{ borderColor: 'var(--border-primary)' }}
                  >
                    {['Off', '10s', '30s', '1m', '5m'].map((opt) => (
                      <button
                        key={opt}
                        className={`w-full text-left px-3 py-1 text-xs transition-colors ${
                          opt === autoRefresh ? 'text-text-primary bg-gray-100 font-medium' : 'text-text-primary hover:bg-gray-50'
                        }`}
                        onClick={() => { setAutoRefresh(opt); setRefreshOpen(false) }}
                      >
                        {opt}
                      </button>
                    ))}
                  </div>
                )}
              </div>
              <div className="relative">
                <button
                  className="flex items-center gap-1.5 px-2 py-1 text-sm text-text-primary hover:bg-gray-100 transition-colors"
                  style={{
                    borderColor: 'var(--border-primary)',
                    borderWidth: 1,
                  }}
                  onClick={() => setTimeOpen(!timeOpen)}
                >
                  <span>{timeRange}</span>
                  <LuChevronDown className="size-3.5" style={{ color: 'var(--text-secondary)' }} />
                </button>
                {timeOpen && (
                  <div
                    className="absolute top-full right-0 mt-1 py-1 min-w-40 rounded border bg-white shadow-md z-50"
                    style={{ borderColor: 'var(--border-primary)' }}
                  >
                    {timeRanges.map((range) => (
                      <button
                        key={range}
                        className={`w-full text-left px-3 py-1.5 text-sm transition-colors ${
                          range === timeRange ? 'text-text-primary bg-gray-100 font-medium' : 'text-text-primary hover:bg-gray-50'
                        }`}
                        onClick={() => { setTimeRange(range); setTimeOpen(false) }}
                      >
                        {range}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </div>
          <div className="flex gap-1.5 px-2 pt-2">
            <div className="flex-1 h-64 rounded border flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
                <span className="text-sm font-semibold text-text-primary">Total Requests</span>
                <div className="flex items-center gap-2">
                  <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                    Count <LuChevronDown className="size-3" />
                  </button>
                  <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                    Grouped by nothing <LuChevronDown className="size-3" />
                  </button>
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
                  <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                    Count <LuChevronDown className="size-3" />
                  </button>
                  <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                    Grouped by nothing <LuChevronDown className="size-3" />
                  </button>
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
                  <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                    Count <LuChevronDown className="size-3" />
                  </button>
                  <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                    Grouped by nothing <LuChevronDown className="size-3" />
                  </button>
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
