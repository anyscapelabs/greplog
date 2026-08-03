import { useState, useMemo } from 'react'
import { useLogs, useFilterState, compileFilterToQuery, parseQueryToChip, chipDisplay, useRefreshControl } from '../hooks/index.ts'
import PageHeader from '../components/PageHeader.tsx'
import FilterSidebar from '../components/FilterSidebar.tsx'
import LogsHistogramChart from '../components/LogsHistogramChart.tsx'
import LogsTable from '../components/LogsTable.tsx'
import LogsDrawer from '../components/LogsDrawer.tsx'
import type { LogEntry } from '../types/index.ts'

export default function Logs() {
  const {
    filters,
    setQuery,
    addChip,
    removeChip,
    toggleService,
    setTimeRange,
    toggleChecked,
    clearAll,
  } = useFilterState()

  const predicate = compileFilterToQuery(filters)
  const {
    logs,
    totalLogs,
    totalRows,
    querySeconds,
    filterSections,
    charts,
    refetch,
    manualRefetch,
  } = useLogs(predicate)

  const {
    isLive,
    toggleLive,
    manualRefresh,
    autoRefresh,
    setAutoRefresh,
  } = useRefreshControl(refetch, { manualRefetch })

  const [filterOpen, setFilterOpen] = useState(true)
  const [drawerLog, setDrawerLog] = useState<LogEntry | null>(null)

  function handleCheck(id: string) {
    const section = filterSections.find((s) => s.items.some((i) => i.id === id))
    if (section?.id === 'service_name') {
      toggleService(id)
    } else {
      toggleChecked(id)
    }
  }

  const checked = useMemo(() => {
    const merged: Record<string, boolean> = { ...filters.checked }
    for (const s of filters.services) {
      merged[s] = true
    }
    return merged
  }, [filters.checked, filters.services])

  function handleQueryKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === 'Enter' && filters.query.trim()) {
      addChip(parseQueryToChip(filters.query.trim()))
      setQuery('')
    }
  }

  function handleRemoveChip(chipStr: string) {
    removeChip(parseQueryToChip(chipStr))
  }

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="Logs"
        showLive
        isLive={isLive}
        onLiveChange={toggleLive}
        onRefresh={manualRefresh}
        showFilterToggle
        showSearch
        timeRange={filters.timeRange}
        onTimeRangeChange={setTimeRange}
        autoRefresh={autoRefresh}
        onAutoRefreshChange={setAutoRefresh}
        filterOpen={filterOpen}
        onFilterToggle={() => setFilterOpen(!filterOpen)}
        chips={filters.chips.map(chipDisplay)}
        query={filters.query}
        onQueryChange={setQuery}
        onQueryKeyDown={handleQueryKeyDown}
        onRemoveChip={handleRemoveChip}
        searchPlaceholder="Search queries..."
        extraActions={
          filters.chips.length > 0 ? (
            <button
              className="text-xs px-2 py-1 rounded hover:bg-[var(--hover-bg)] transition-colors"
              style={{ color: 'var(--accent)' }}
              onClick={clearAll}
            >
              Clear filters
            </button>
          ) : undefined
        }
      />
      <div className="flex flex-1 min-h-0 relative">
        {filterOpen && <FilterSidebar checked={checked} onCheck={handleCheck} sections={filterSections} />}
        <div className="flex-1 flex flex-col min-w-0">
          <div className="flex gap-1.5 px-2 pt-2">
            <div
              className="flex-1 min-h-40 h-56 max-h-[70vh] resize-y overflow-hidden rounded border flex flex-col"
              style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}
            >
              <div className="flex items-center justify-between px-2 py-1.5 border-b shrink-0" style={{ borderColor: 'var(--border-primary)' }}>
                <span className="text-sm font-semibold text-text-primary">Logs Histogram</span>
              </div>
              <div className="flex-1 min-h-0">
                <LogsHistogramChart data={charts.logsHistogram} />
              </div>
            </div>
          </div>
          <LogsTable data={logs} totalRows={totalRows} totalLogs={totalLogs} querySeconds={querySeconds} onView={setDrawerLog} />
        </div>
      </div>
      <LogsDrawer open={!!drawerLog} onClose={() => setDrawerLog(null)} log={drawerLog} />
    </div>
  )
}
