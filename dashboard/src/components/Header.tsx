import { useState } from 'react'
import wordmark from '../assets/wordmark-white.svg'
import { FaChartDiagram } from 'react-icons/fa6'
import { PiBroadcastFill } from 'react-icons/pi'
import {
  RiArrowDownSLine,
  RiArrowLeftSLine,
  RiArrowRightSLine,
  RiExternalLinkLine,
  RiFileListLine,
  RiRefreshLine,
  RiTimeLine,
} from 'react-icons/ri'

const TIME_RANGES = [
  { label: 'Last 15 minutes', value: '15m' },
  { label: 'Last 1 hour', value: '1h' },
  { label: 'Last 3 hours', value: '3h' },
  { label: 'Last 6 hours', value: '6h' },
  { label: 'Last 12 hours', value: '12h' },
  { label: 'Last 24 hours', value: '24h' },
  { label: 'Last 7 days', value: '7d' },
  { label: 'Last 30 days', value: '30d' },
]

const REFRESH_OPTIONS = [
  { label: 'Off', value: 'off' },
  { label: '5s', value: '5s' },
  { label: '10s', value: '10s' },
  { label: '30s', value: '30s' },
  { label: '1m', value: '1m' },
]

export type TimeRange = '15m' | '1h' | '3h' | '6h' | '12h' | '24h' | '7d' | '30d'

export type TabId = 'logs' | 'metrics' | 'tail'

interface HeaderProps {
  activeTab: TabId
  onTabChange: (tab: TabId) => void
  range: TimeRange
  onRangeChange: (range: TimeRange) => void
  liveTailActive?: boolean
  onLiveTailToggle?: () => void
  refreshInterval?: string
  onRefreshIntervalChange?: (value: string) => void
  onManualRefresh?: () => void
}

function getSelectedTimeRange(range: TimeRange) {
  const matched = TIME_RANGES.find((timeRangeOption) => timeRangeOption.value === range)

  if (matched) return matched

  return TIME_RANGES[1]
}

function getRefreshIntervalLabel(refreshInterval: string): string {
  const matched = REFRESH_OPTIONS.find((refreshOption) => refreshOption.value === refreshInterval)

  if (matched) return matched.label

  return 'Off'
}

function getTabClassName(tab: TabId, activeTab: TabId): string {
  const base = 'flex cursor-pointer items-center gap-1.5 text-sm font-medium transition-colors'

  if (tab === activeTab) return `${base} text-zinc-100`

  return `${base} text-zinc-400 hover:text-zinc-100`
}

function getLiveTailClassName(isActive: boolean): string {
  const base = 'flex cursor-pointer items-center gap-1.5 rounded-md border px-3 py-1 text-sm font-medium transition-colors'

  if (isActive) return `${base} border-[#16a34a] bg-[#16a34a] text-white`

  return `${base} border-zinc-700 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100`
}

function getRefreshArrowClassName(isOpen: boolean): string {
  if (isOpen) return 'h-4 w-4 rotate-180 transition-transform'

  return 'h-4 w-4 transition-transform'
}

function getRangeArrowClassName(isOpen: boolean): string {
  if (isOpen) return 'h-4 w-4 rotate-180 transition-transform'

  return 'h-4 w-4 transition-transform'
}

function Header({
  activeTab,
  onTabChange,
  range,
  onRangeChange,
  liveTailActive = false,
  onLiveTailToggle,
  refreshInterval = 'off',
  onRefreshIntervalChange,
  onManualRefresh,
}: HeaderProps) {
  const [isRangeOpen, setIsRangeOpen] = useState(false)
  const [isRefreshOpen, setIsRefreshOpen] = useState(false)
  const [isRefreshing, setIsRefreshing] = useState(false)

  const selectedTimeRange = getSelectedTimeRange(range)
  const selectedRangeIndex = TIME_RANGES.findIndex((timeRangeOption) => timeRangeOption.value === range)
  const refreshLabel = getRefreshIntervalLabel(refreshInterval)

  const handleShiftRange = (delta: number) => {
    if (!onRangeChange) return

    const baseIndex = selectedRangeIndex === -1 ? 0 : selectedRangeIndex
    const nextIndex = (baseIndex + delta + TIME_RANGES.length) % TIME_RANGES.length
    const nextRange = TIME_RANGES[nextIndex]

    if (!nextRange) return

    onRangeChange(nextRange.value as TimeRange)
  }

  const handleLiveTailClick = () => {
    onTabChange('logs')

    if (!onLiveTailToggle) return

    onLiveTailToggle()
  }

  const handleManualRefresh = () => {
    if (isRefreshing) return

    if (!onManualRefresh) return

    setIsRefreshing(true)
    onManualRefresh()
    setTimeout(() => setIsRefreshing(false), 800)
  }

  const handleRefreshIntervalSelect = (value: string) => {
    if (!value) return

    if (!onRefreshIntervalChange) return

    onRefreshIntervalChange(value)
    setIsRefreshOpen(false)
  }

  const handleRangeSelect = (value: string) => {
    if (!value) return

    onRangeChange(value as TimeRange)
    setIsRangeOpen(false)
  }

  if (!activeTab) return null

  if (!range) return null

  return (
    <header className="flex items-center justify-between border-b border-zinc-800 px-3 py-2">
      <img src={wordmark} alt="Greplog" className="h-6 w-auto" />
      <div className="flex items-center gap-8">
        <nav className="flex items-center gap-5">
          <button type="button" onClick={() => onTabChange('logs')} className={getTabClassName('logs', activeTab)}>
            <RiFileListLine className="h-4 w-4" />
            Log Explorer
          </button>
          <button type="button" onClick={() => onTabChange('metrics')} className={getTabClassName('metrics', activeTab)}>
            <FaChartDiagram className="h-4 w-4" />
            Metrics
          </button>
        </nav>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handleLiveTailClick}
            className={getLiveTailClassName(liveTailActive)}
          >
            <PiBroadcastFill className="h-4 w-4" />
            Live tail
          </button>
          <div className="relative flex items-center gap-1">
            <button
              type="button"
              onClick={handleManualRefresh}
              className="flex items-center gap-1.5 rounded-md border border-zinc-700 px-3 py-1 text-sm font-medium text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
            >
              <RiRefreshLine className={`h-4 w-4 transition-transform ${isRefreshing ? 'animate-spin' : ''}`} />
              Refresh
            </button>
            <div className="relative">
              <button
                type="button"
                onClick={() => setIsRefreshOpen((previousOpen) => !previousOpen)}
                className="flex items-center gap-1 rounded-md border border-zinc-700 px-3 py-1 text-sm font-medium text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
              >
                Auto: {refreshLabel}
                <RiArrowDownSLine className={getRefreshArrowClassName(isRefreshOpen)} />
              </button>
              {isRefreshOpen && (
                <>
                  <div className="fixed inset-0 z-10" onClick={() => setIsRefreshOpen(false)} />
                  <ul className="absolute right-0 top-full z-20 mt-1 w-32 rounded-md border border-zinc-700 bg-zinc-900 py-1 text-xs shadow-lg">
                    {REFRESH_OPTIONS.map((refreshOption) => (
                      <li key={refreshOption.value}>
                        <button
                          type="button"
                          onClick={() => handleRefreshIntervalSelect(refreshOption.value)}
                          className={`flex w-full items-center px-3 py-1.5 text-left transition-colors hover:bg-zinc-800 hover:text-zinc-100 ${refreshOption.value === refreshInterval ? 'text-zinc-100' : 'text-zinc-400'}`}
                        >
                          {refreshOption.label}
                        </button>
                      </li>
                    ))}
                  </ul>
                </>
              )}
            </div>
          </div>
          <div className="relative">
          <div className="flex items-center overflow-hidden rounded-md border border-zinc-700">
            <button
              type="button"
              onClick={() => handleShiftRange(-1)}
              className="flex items-center px-2 py-1 text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
            >
              <RiArrowLeftSLine className="h-5 w-5" />
            </button>
            <button
              type="button"
              onClick={() => setIsRangeOpen((previousOpen) => !previousOpen)}
              className="flex w-48 items-center justify-center gap-1.5 border-x border-zinc-700 px-3 py-1 text-sm font-medium text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
            >
              <RiTimeLine className="h-4 w-4" />
              {selectedTimeRange.label}
              <RiArrowDownSLine className={getRangeArrowClassName(isRangeOpen)} />
            </button>
            <button
              type="button"
              onClick={() => handleShiftRange(1)}
              className="flex items-center px-2 py-1 text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
            >
              <RiArrowRightSLine className="h-5 w-5" />
            </button>
          </div>
            {isRangeOpen && (
              <>
                <div
                  className="fixed inset-0 z-10"
                  onClick={() => setIsRangeOpen(false)}
                />
                <ul className="absolute right-0 top-full z-20 mt-1 w-44 rounded-md border border-zinc-700 bg-zinc-900 py-1 text-sm shadow-lg">
                  {TIME_RANGES.map((timeRangeOption) => (
                    <li key={timeRangeOption.value}>
                      <button
                        type="button"
                        onClick={() => handleRangeSelect(timeRangeOption.value)}
                        className={`flex w-full items-center px-3 py-1.5 text-left transition-colors hover:bg-zinc-800 hover:text-zinc-100 ${
                          timeRangeOption.value === range ? 'text-zinc-100' : 'text-zinc-400'
                        }`}
                      >
                        {timeRangeOption.label}
                      </button>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </div>
        </div>
        <a
          href="https://docs.greplog.dev"
          target="_blank"
          rel="noreferrer"
          className="flex items-center gap-1.5 text-sm font-medium text-zinc-300 transition-colors hover:text-zinc-100"
        >
          Documentation
          <RiExternalLinkLine className="h-4 w-4" />
        </a>
      </div>
    </header>
  )
}

export default Header
