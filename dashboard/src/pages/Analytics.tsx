import { useState } from 'react'
import { useFilterState } from '../hooks/index.ts'
import PageHeader from '../components/PageHeader.tsx'

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
    </div>
  )
}
