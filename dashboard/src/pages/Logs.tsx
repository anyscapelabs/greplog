import { useState, useEffect, useMemo } from 'react'
import { useLogs, useFilterState, compileFilterToQuery, parseQueryToChip, chipDisplay, useRefreshControl, useDebouncedValue, useFilterSidebarOpen } from '../hooks/index.ts'
import PageHeader from '../components/PageHeader.tsx'
import FilterSidebar from '../components/FilterSidebar.tsx'
import { LogsHistogramChart } from '../components/LogsHistogramChart.tsx'
import LogsTable from '../components/LogsTable.tsx'
import LogsDrawer from '../components/LogsDrawer.tsx'
import TopLoadingBar from '../components/TopLoadingBar.tsx'
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

  const debouncedQuery = useDebouncedValue(filters.query, 300)
  const predicate = compileFilterToQuery(filters, debouncedQuery)
  // Sidebar facet counts come from the base population (search text, chips,
  // time range) with every facet selection excluded, so marking a filter never
  // removes the other options in any section and multiple filters can be set.
  const facetPredicate = compileFilterToQuery(filters, debouncedQuery, {
    excludeCheckedSections: ['log_level', 'service_name', 'status_code', 'response_status'],
    excludeServices: true,
    excludeLogLevels: true,
  })
  const [limit, setLimit] = useState('500')
  const [page, setPage] = useState(0)
  const [sortDirection, setSortDirection] = useState<'asc' | 'desc'>('desc')
  const {
    logs,
    totalLogs,
    totalRows,
    querySeconds,
    filterSections,
    charts,
    isFetching,
    isLoading,
    refetch,
    manualRefetch,
  } = useLogs(predicate, filters.timeRange, {
    limit: parseInt(limit.replace('k', '000'), 10),
    page,
    sortDirection,
    facetPredicate,
  })

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setPage(0)
  }, [filters])

  // Preserve previous chart data while loading new data to avoid flicker.
  // When fetching completes, sync the display to show the latest data.
  const [displayCharts, setDisplayCharts] = useState(charts)
  useEffect(() => {
    if (!isFetching) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setDisplayCharts(charts)
    }
  }, [isFetching, charts])

  const {
    isLive,
    toggleLive,
    manualRefresh,
    autoRefresh,
    setAutoRefresh,
  } = useRefreshControl(refetch, { manualRefetch })

  const [filterOpen, setFilterOpen] = useFilterSidebarOpen()
  const [drawerLog, setDrawerLog] = useState<LogEntry | null>(null)

  function handleCheck(sectionId: string, id: string) {
    if (sectionId === 'service_name') {
      toggleService(id)
    } else {
      toggleChecked(sectionId, id)
    }
  }

  const checked = useMemo(() => {
    const merged: Record<string, boolean> = {}
    for (const s of filters.services) {
      merged[s] = true
    }
    for (const ids of Object.values(filters.checked)) {
      for (const id of ids) {
        merged[id] = true
      }
    }
    return merged
  }, [filters.checked, filters.services])

  // Ensure consistent colors across all chart modes (bars and areas)
  // Colors are derived from log levels and should match both rendering styles

  function handleQueryKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === 'Enter' && filters.query.trim()) {
      addChip(parseQueryToChip(filters.query.trim()))
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
              className="text-xs px-2 py-1 rounded hover:bg-(--hover-bg) transition-colors"
              style={{ color: 'var(--accent)' }}
              onClick={clearAll}
            >
              Clear filters
            </button>
          ) : undefined
        }
      />
      <div className="flex flex-1 min-h-0 relative">
        {filterOpen && <FilterSidebar checked={checked} onCheck={handleCheck} sections={filterSections} loading={isLoading} />}
        <div className="flex-1 flex flex-col min-w-0">
          <div className="flex gap-1.5 px-2 pt-2 pb-1">
            <div
              className="relative flex-1 min-h-56 h-72 max-h-[75vh] resize-y overflow-hidden rounded border flex flex-col p-2"
              style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}
            >
              <TopLoadingBar active={isFetching} />
              <div className="flex items-center justify-between px-2 py-1 border-b shrink-0" style={{ borderColor: 'var(--border-primary)' }}>
                <span className="text-sm font-semibold text-text-primary">Logs Histogram</span>
              </div>
              <div className="flex-1 min-h-0 overflow-hidden">
                <LogsHistogramChart data={displayCharts.logsHistogram} />
              </div>
            </div>
          </div>
          <LogsTable
            data={logs}
            totalRows={totalRows}
            totalLogs={totalLogs}
            querySeconds={querySeconds}
            limit={limit}
            page={page}
            sortDirection={sortDirection}
            onLimitChange={(next) => {
              setLimit(next)
              setPage(0)
            }}
            onPageChange={setPage}
            onSortDirectionChange={(next) => {
              setSortDirection(next)
              setPage(0)
            }}
            onView={setDrawerLog}
            isFetching={isFetching}
          />
        </div>
      </div>
      <LogsDrawer open={!!drawerLog} onClose={() => setDrawerLog(null)} log={drawerLog} />
    </div>
  )
}
