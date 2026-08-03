import { useState, useMemo } from 'react'
import { useErrors, useFilterState, compileFilterToQuery, parseQueryToChip, chipDisplay, useRefreshControl } from '../hooks/index.ts'
import PageHeader from '../components/PageHeader.tsx'
import FilterSidebar from '../components/FilterSidebar.tsx'
import ErrorCountChart from '../components/ErrorCountChart.tsx'
import ErrorRateChart from '../components/ErrorRateChart.tsx'
import ErrorByServiceChart from '../components/ErrorByServiceChart.tsx'
import ErrorsTable from '../components/ErrorsTable.tsx'
import ErrorsDrawer from '../components/ErrorsDrawer.tsx'
import Dropdown from '../components/Dropdown.tsx'
import type { ErrorEntry } from '../types/index.ts'

export default function Errors() {

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
  // Sidebar facet counts come from the base population (search text, chips,
  // time range) with every facet selection excluded, so marking a filter never
  // removes the other options in any section and multiple filters can be set.
  const facetPredicate = compileFilterToQuery(filters, undefined, {
    excludeCheckedSections: ['service_name', 'error_type', 'log_level', 'status_code', 'response_status'],
    excludeServices: true,
    excludeLogLevels: true,
  })
  const {
    errors,
    totalErrors,
    totalRows,
    querySeconds,
    filterSections,
    chartMetrics,
    groupByOptions,
    charts,
    refetch,
    manualRefetch,
    isLoading,
  } = useErrors(predicate, facetPredicate)

  const {
    isLive,
    toggleLive,
    manualRefresh,
    autoRefresh,
    setAutoRefresh,
  } = useRefreshControl(refetch, { manualRefetch })

  const [filterOpen, setFilterOpen] = useState(true)
  const [chart1Metric, setChart1Metric] = useState('count')
  const [chart1Group, setChart1Group] = useState('nothing')
  const [chart2Metric, setChart2Metric] = useState('count')
  const [chart2Group, setChart2Group] = useState('nothing')
  const [chart3Metric, setChart3Metric] = useState('count')
  const [chart3Group, setChart3Group] = useState('nothing')
  const [drawerError, setDrawerError] = useState<ErrorEntry | null>(null)

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
        title="Errors"
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
        searchPlaceholder="Search errors..."
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
        {filterOpen && <FilterSidebar checked={checked} onCheck={handleCheck} sections={filterSections} loading={isLoading} />}
        <div className="flex-1 flex flex-col min-w-0">
          <div className="flex gap-1.5 px-2 pt-2">
            <div className="flex-1 h-64 rounded border flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
                <span className="text-sm font-semibold text-text-primary">Error Count</span>
                <div className="flex items-center gap-2">
                  <Dropdown
                    trigger={<span className="text-xs text-text-secondary">{chartMetrics.find(m => m.value === chart1Metric)?.label}</span>}
                    items={chartMetrics}
                    value={chart1Metric}
                    onChange={setChart1Metric}
                    minWidth="min-w-20"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                  <Dropdown
                    trigger={<span className="text-xs text-text-secondary">Grouped by {groupByOptions.errorCount.find(g => g.value === chart1Group)?.label}</span>}
                    items={groupByOptions.errorCount}
                    value={chart1Group}
                    onChange={setChart1Group}
                    minWidth="min-w-36"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                </div>
              </div>
              <div className="flex-1 p-1">
                <ErrorCountChart metric={chart1Metric} groupBy={chart1Group} data={charts.countTimeseries} />
              </div>
            </div>
            <div className="flex-1 h-64 rounded border flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
                <span className="text-sm font-semibold text-text-primary">Error Rate</span>
                <div className="flex items-center gap-2">
                  <Dropdown
                    trigger={<span className="text-xs text-text-secondary">{chartMetrics.find(m => m.value === chart2Metric)?.label}</span>}
                    items={chartMetrics}
                    value={chart2Metric}
                    onChange={setChart2Metric}
                    minWidth="min-w-20"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                  <Dropdown
                    trigger={<span className="text-xs text-text-secondary">Grouped by {groupByOptions.errorRate.find(g => g.value === chart2Group)?.label}</span>}
                    items={groupByOptions.errorRate}
                    value={chart2Group}
                    onChange={setChart2Group}
                    minWidth="min-w-36"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                </div>
              </div>
              <div className="flex-1 p-1">
                <ErrorRateChart metric={chart2Metric} groupBy={chart2Group} data={charts.rateTimeseries} />
              </div>
            </div>
            <div className="flex-1 h-64 rounded border flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
                <span className="text-sm font-semibold text-text-primary">Errors by Service</span>
                <div className="flex items-center gap-2">
                  <Dropdown
                    trigger={<span className="text-xs text-text-secondary">{chartMetrics.find(m => m.value === chart3Metric)?.label}</span>}
                    items={chartMetrics}
                    value={chart3Metric}
                    onChange={setChart3Metric}
                    minWidth="min-w-20"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                  <Dropdown
                    trigger={<span className="text-xs text-text-secondary">Grouped by {groupByOptions.byService.find(g => g.value === chart3Group)?.label}</span>}
                    items={groupByOptions.byService}
                    value={chart3Group}
                    onChange={setChart3Group}
                    minWidth="min-w-36"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                </div>
              </div>
              <div className="flex-1 p-1">
                <ErrorByServiceChart metric={chart3Metric} groupBy={chart3Group} data={charts.byServiceDistribution} />
              </div>
            </div>
          </div>
          <ErrorsTable data={errors} totalRows={totalRows} totalLogs={totalErrors} querySeconds={querySeconds} onView={setDrawerError} />
        </div>
      </div>
      <ErrorsDrawer open={!!drawerError} onClose={() => setDrawerError(null)} error={drawerError} />
    </div>
  )
}
