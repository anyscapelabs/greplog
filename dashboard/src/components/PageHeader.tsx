import { useState, type ReactNode } from 'react'
import { LuRefreshCw, LuCircleDot, LuPanelLeftClose, LuPanelLeftOpen, LuServer } from 'react-icons/lu'
import SearchInput from './SearchInput.tsx'
import Dropdown from './Dropdown.tsx'

const timeRanges = ['Last 15 min', 'Last 1 hour', 'Last 6 hours', 'Last 24 hours', 'Last 7 days', 'Custom']
const autoRefreshOptions = ['Off', '10s', '30s', '1m', '5m']
const defaultServices = ['All Services', 'web', 'api', 'db', 'worker']

interface PageHeaderProps {
  title: string
  showLive?: boolean
  showFilterToggle?: boolean
  showService?: boolean
  showSearch?: boolean
  timeRange: string
  onTimeRangeChange: (value: string) => void
  autoRefresh: string
  onAutoRefreshChange: (value: string) => void
  services?: string[]
  service?: string
  onServiceChange?: (value: string) => void
  filterOpen?: boolean
  onFilterToggle?: () => void
  chips?: string[]
  query?: string
  onQueryChange?: (value: string) => void
  onQueryKeyDown?: (e: React.KeyboardEvent<HTMLInputElement>) => void
  onRemoveChip?: (chip: string) => void
  searchPlaceholder?: string
  extraActions?: ReactNode
}

export default function PageHeader({
  title,
  showLive,
  showFilterToggle,
  showService,
  showSearch,
  timeRange,
  onTimeRangeChange,
  autoRefresh,
  onAutoRefreshChange,
  services = defaultServices,
  service = 'All Services',
  onServiceChange,
  filterOpen,
  onFilterToggle,
  chips = [],
  query = '',
  onQueryChange,
  onQueryKeyDown,
  onRemoveChip,
  searchPlaceholder,
  extraActions,
}: PageHeaderProps) {
  const [spinning, setSpinning] = useState(false)
  const [live, setLive] = useState(false)

  return (
    <>
      <div
        className="flex items-center px-4 h-12 shrink-0 border-b gap-3"
        style={{
          backgroundColor: 'var(--bg-secondary)',
          borderColor: 'var(--border-primary)',
        }}
      >
        <span className="text-2xl font-semibold flex items-center gap-2">
          <span style={{ color: 'var(--text-secondary)' }}>Grep</span>
          <span className="text-text-primary">{title}</span>
        </span>
        <div className="ml-auto flex items-center gap-2">
          {extraActions}
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
          {showLive && (
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
          )}
        </div>
      </div>
      <div
        className="flex items-center h-10 border-b shrink-0 gap-0"
        style={{
          backgroundColor: 'var(--bg-secondary)',
          borderColor: 'var(--border-primary)',
        }}
      >
        {showFilterToggle && (
          <button
            className="flex items-center justify-center p-1.5 text-text-primary hover:bg-[var(--hover-bg)] transition-colors ml-1"
            onClick={onFilterToggle}
            title={filterOpen ? 'Close filters' : 'Open filters'}
          >
            {filterOpen ? <LuPanelLeftClose className="size-4" /> : <LuPanelLeftOpen className="size-4" />}
          </button>
        )}
        {showFilterToggle && <div className="h-5 w-px mx-2" style={{ backgroundColor: 'var(--border-primary)' }} />}
        {showService && onServiceChange && (
          <>
            <Dropdown
              trigger={<><LuServer className="size-3.5" style={{ color: 'var(--text-secondary)' }} /><span>{service}</span></>}
              items={services.map((s) => ({ label: s, value: s }))}
              value={service}
              onChange={onServiceChange}
              minWidth="min-w-32"
            />
            <div className="h-5 w-px mx-2" style={{ backgroundColor: 'var(--border-primary)' }} />
          </>
        )}
        {showSearch && onQueryChange && onQueryKeyDown && onRemoveChip && (
          <>
            <SearchInput
              chips={chips}
              query={query}
              onQueryChange={onQueryChange}
              onKeyDown={onQueryKeyDown}
              onRemoveChip={onRemoveChip}
              placeholder={searchPlaceholder}
            />
            <div className="ml-auto flex items-center gap-2 pr-4">
              <Dropdown
                trigger={<><span className="text-text-primary text-sm">Auto refresh</span>{autoRefresh !== 'Off' && <span className="flex items-center justify-center px-1.5 py-0.5 text-xs text-text-primary bg-[var(--bg-primary)] rounded">{autoRefresh}</span>}</>}
                items={autoRefreshOptions.map((opt) => ({ label: opt, value: opt }))}
                value={autoRefresh}
                onChange={onAutoRefreshChange}
                align="right"
                minWidth="min-w-16"
              />
              <Dropdown
                trigger={<span>{timeRange}</span>}
                items={timeRanges.map((r) => ({ label: r, value: r }))}
                value={timeRange}
                onChange={onTimeRangeChange}
                align="right"
                minWidth="min-w-40"
              />
            </div>
          </>
        )}
      </div>
    </>
  )
}
