import { useState, useMemo } from 'react'
import { useLogs, useFilterState, compileFilterToQuery, parseQueryToChip, chipDisplay, useRefreshControl } from '../hooks/index.ts'
import { useAgent } from '../context/AgentContext.tsx'
import PageHeader from '../components/PageHeader.tsx'
import FilterSidebar from '../components/FilterSidebar.tsx'
import LogVolumeChart from '../components/LogVolumeChart.tsx'
import ErrorsChart from '../components/ErrorsChart.tsx'
import StatusCodesChart from '../components/StatusCodesChart.tsx'
import LogsTable from '../components/LogsTable.tsx'
import LogsDrawer from '../components/LogsDrawer.tsx'
import WaitingOverlay, { SDK_SETUP_TERMINAL } from '../components/WaitingOverlay.tsx'
import Dropdown from '../components/Dropdown.tsx'

export default function Logs() {
  const { connected } = useAgent()

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
    chartMetrics,
    groupByOptions,
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
  } = useRefreshControl(refetch)

  const [filterOpen, setFilterOpen] = useState(true)
  const [chart1Metric, setChart1Metric] = useState('count')
  const [chart1Group, setChart1Group] = useState('nothing')
  const [chart2Metric, setChart2Metric] = useState('count')
  const [chart2Group, setChart2Group] = useState('nothing')
  const [chart3Metric, setChart3Metric] = useState('count')
  const [chart3Group, setChart3Group] = useState('nothing')
  const [drawerLog, setDrawerLog] = useState<any>(null)

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
          <WaitingOverlay
            visible={!connected}
            message="Run the Greplog agent and configure an SDK to start collecting logs"
            terminal={SDK_SETUP_TERMINAL}
          />
        {filterOpen && <FilterSidebar checked={checked} onCheck={handleCheck} sections={filterSections} />}
        <div className="flex-1 flex flex-col min-w-0">
          <div className="flex gap-1.5 px-2 pt-2">
            <div className="flex-1 h-64 rounded border flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
                <span className="text-sm font-semibold text-text-primary">Total Requests</span>
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
                    trigger={<span className="text-xs text-text-secondary">Grouped by {groupByOptions.requests.find(g => g.value === chart1Group)?.label}</span>}
                    items={groupByOptions.requests}
                    value={chart1Group}
                    onChange={setChart1Group}
                    minWidth="min-w-36"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                </div>
              </div>
              <div className="flex-1 p-1">
                <LogVolumeChart metric={chart1Metric} groupBy={chart1Group} />
              </div>
            </div>
            <div className="flex-1 h-64 rounded border flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
                <span className="text-sm font-semibold text-text-primary">Errors</span>
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
                    trigger={<span className="text-xs text-text-secondary">Grouped by {groupByOptions.errors.find(g => g.value === chart2Group)?.label}</span>}
                    items={groupByOptions.errors}
                    value={chart2Group}
                    onChange={setChart2Group}
                    minWidth="min-w-36"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                </div>
              </div>
              <div className="flex-1 p-1">
                <ErrorsChart metric={chart2Metric} groupBy={chart2Group} />
              </div>
            </div>
            <div className="flex-1 h-64 rounded border flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
                <span className="text-sm font-semibold text-text-primary">Status Codes</span>
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
                    trigger={<span className="text-xs text-text-secondary">Grouped by {groupByOptions.statusCodes.find(g => g.value === chart3Group)?.label}</span>}
                    items={groupByOptions.statusCodes}
                    value={chart3Group}
                    onChange={setChart3Group}
                    minWidth="min-w-36"
                    showChevron
                    triggerClassName="text-xs text-text-secondary hover:text-text-primary"
                  />
                </div>
              </div>
              <div className="flex-1 p-1">
                <StatusCodesChart metric={chart3Metric} groupBy={chart3Group} />
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
