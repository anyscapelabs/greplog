import { useState } from 'react'
import { LuClock } from 'react-icons/lu'
import Dropdown from './Dropdown.tsx'

const TIME_RANGES = [
  { label: 'Last 24 hours', value: 'Last 24 hours' },
  { label: '15 minutes', value: '15 minutes' },
  { label: '6 hours', value: '6 hours' },
  { label: 'Last 7 days', value: 'Last 7 days' },
]

export default function TotalEventsCard() {
  const [timeRange, setTimeRange] = useState('Last 24 hours')

  return (
    <div
      className="min-h-32 flex flex-col"
      style={{
        backgroundColor: 'var(--bg-secondary)',
        border: '1px solid var(--border-primary)',
        borderRadius: '10px',
      }}
    >
      <div className="flex items-center justify-between p-2">
        <span className="text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          Total Events
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
    </div>
  )
}