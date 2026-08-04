import { useState } from 'react'
import { LuClock } from 'react-icons/lu'
import Dropdown from './Dropdown.tsx'
import { formatCompact } from '../lib/formatCompact.ts'

const TIME_RANGES = [
  { label: 'Last 6 hours', value: 'Last 6 hours' },
  { label: 'Last 15 minutes', value: 'Last 15 minutes' },
  { label: 'Last 3 hours', value: 'Last 3 hours' },
  { label: 'Last 7 days', value: 'Last 7 days' },
  { label: 'Last 30 days', value: 'Last 30 days' },
]

const MOCK_SERVICES = [
  { name: 'web', events: 128_000 },
  { name: 'api', events: 96_500 },
  { name: 'db', events: 42_300 },
  { name: 'worker', events: 18_700 },
]

export default function ServicesCard() {
  const [timeRange, setTimeRange] = useState('Last 6 hours')

  return (
    <div
      className="min-h-24 flex flex-col"
      style={{
        backgroundColor: 'var(--bg-secondary)',
        border: '1px solid var(--border-primary)',
        borderRadius: '10px',
      }}
    >
      <div className="flex items-center justify-between p-2">
        <span className="text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          Services
        </span>
        <Dropdown
          trigger={
            <span className="flex items-center gap-1.5" style={{ color: 'var(--accent)' }}>
              <LuClock className="size-3.5" />
              {timeRange}
            </span>
          }
          items={TIME_RANGES}
          value={timeRange}
          onChange={setTimeRange}
          align="right"
          minWidth="min-w-32"
          triggerClassName="px-2 py-1 text-xs hover:bg-[var(--hover-bg)] rounded"
        />
      </div>
      <div className="border-b" style={{ borderColor: 'var(--border-primary)' }} />
      <div
        className="flex-1 relative overflow-hidden"
        style={{
          backgroundColor: 'var(--accent)',
          borderBottomLeftRadius: '10px',
          borderBottomRightRadius: '10px',
        }}
      >
        <div className="absolute inset-0 flex flex-col justify-center gap-1 px-3">
          {MOCK_SERVICES.map((s) => (
            <div key={s.name} className="flex items-center justify-between">
              <span className="font-mono text-sm" style={{ color: '#ffffff' }}>
                {s.name}
              </span>
              <span className="font-mono text-sm font-bold tabular-nums" style={{ color: '#ffffff' }}>
                {formatCompact(s.events)}
              </span>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}