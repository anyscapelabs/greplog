import { useState } from 'react'
import { useFilterState } from '../hooks/index.ts'
import PageHeader from '../components/PageHeader.tsx'
import TotalEventsCard from '../components/TotalEventsCard.tsx'

export default function Analytics() {
  const { filters, setTimeRange } = useFilterState()
  const [autoRefresh, setAutoRefresh] = useState('Off')

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="Analytics"
        timeRange={filters.timeRange}
        onTimeRangeChange={setTimeRange}
        autoRefresh={autoRefresh}
        onAutoRefreshChange={setAutoRefresh}
      />
      <div className="flex-1 overflow-y-auto p-2 relative">
        <div className="flex gap-0.5">
          <div className="w-72 shrink-0">
            <TotalEventsCard />
          </div>
        </div>
      </div>
    </div>
  )
}
