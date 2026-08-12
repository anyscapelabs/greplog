import { useState } from 'react'
import wordmark from '../assets/wordmark-white.svg'
import { FaChartDiagram } from 'react-icons/fa6'
import {
  RiArrowDownSLine,
  RiArrowLeftSLine,
  RiArrowRightSLine,
  RiExternalLinkLine,
  RiFileListLine,
  RiRadioLine,
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

export type TimeRange = '15m' | '1h' | '3h' | '6h' | '12h' | '24h' | '7d' | '30d'

export type TabId = 'logs' | 'metrics' | 'tail'

interface HeaderProps {
  activeTab: TabId
  onTabChange: (tab: TabId) => void
  range: TimeRange
  onRangeChange: (range: TimeRange) => void
}

function Header({
  activeTab,
  onTabChange,
  range,
  onRangeChange,
}: HeaderProps) {
  const [rangeOpen, setRangeOpen] = useState(false)
  const selected =
    TIME_RANGES.find((r) => r.value === range) ?? TIME_RANGES[1]

  const currentIndex = TIME_RANGES.findIndex((r) => r.value === range)

  const shiftRange = (delta: number) => {
    const base = currentIndex === -1 ? 0 : currentIndex
    const next =
      (base + delta + TIME_RANGES.length) % TIME_RANGES.length
    onRangeChange(TIME_RANGES[next].value as TimeRange)
  }

  const tabClass = (tab: TabId) =>
    `flex cursor-pointer items-center gap-1.5 text-sm font-medium transition-colors ${
      tab === activeTab
        ? 'text-zinc-100'
        : 'text-zinc-400 hover:text-zinc-100'
    }`

  return (
    <header className="flex items-center justify-between border-b border-zinc-800 px-3 py-2">
      <img src={wordmark} alt="Greplog" className="h-6 w-auto" />
      <div className="flex items-center gap-8">
        <nav className="flex items-center gap-5">
          <button type="button" onClick={() => onTabChange('logs')} className={tabClass('logs')}>
            <RiFileListLine className="h-4 w-4" />
            Log Explorer
          </button>
          <button type="button" onClick={() => onTabChange('metrics')} className={tabClass('metrics')}>
            <FaChartDiagram className="h-4 w-4" />
            Metrics
          </button>
          <button type="button" onClick={() => onTabChange('tail')} className={tabClass('tail')}>
            <RiRadioLine className="h-4 w-4" />
            Live tail
          </button>
        </nav>
        <div className="relative">
          <div className="flex items-center overflow-hidden rounded-md border border-zinc-700">
            <button
              type="button"
              onClick={() => shiftRange(-1)}
              className="flex items-center px-2 py-1 text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
            >
              <RiArrowLeftSLine className="h-5 w-5" />
            </button>
            <button
              type="button"
              onClick={() => setRangeOpen((open) => !open)}
              className="flex w-48 items-center justify-center gap-1.5 border-x border-zinc-700 px-3 py-1 text-sm font-medium text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
            >
              <RiTimeLine className="h-4 w-4" />
              {selected.label}
              <RiArrowDownSLine className="h-4 w-4" />
            </button>
            <button
              type="button"
              onClick={() => shiftRange(1)}
              className="flex items-center px-2 py-1 text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
            >
              <RiArrowRightSLine className="h-5 w-5" />
            </button>
          </div>
          {rangeOpen && (
            <>
              <div
                className="fixed inset-0 z-10"
                onClick={() => setRangeOpen(false)}
              />
              <ul className="absolute right-0 top-full z-20 mt-1 w-44 rounded-md border border-zinc-700 bg-zinc-900 py-1 text-sm shadow-lg">
                {TIME_RANGES.map((r) => (
                  <li key={r.value}>
                    <button
                      type="button"
                      onClick={() => {
                        onRangeChange(r.value as TimeRange)
                        setRangeOpen(false)
                      }}
                      className={`flex w-full items-center px-3 py-1.5 text-left transition-colors hover:bg-zinc-800 hover:text-zinc-100 ${
                        r.value === range ? 'text-zinc-100' : 'text-zinc-400'
                      }`}
                    >
                      {r.label}
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
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