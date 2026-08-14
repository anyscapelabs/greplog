import { useEffect, useMemo, useState } from 'react'
import { RiPlayFill } from 'react-icons/ri'
import SearchBar from '../components/SearchBar'
import ServiceSelect from '../components/ServiceSelect'
import FiltersSidebar from '../components/logs/FiltersSidebar'
import LogsList from '../components/logs/LogsList'
import Timeline from '../components/logs/Timeline'
import { useLogExplorer } from '../hooks/useLogs'
import type { QueryFilters } from '../api/logs'
import { extractSeverity } from '../api/logs'
import type { TimeRange } from '../components/Header'

interface LogExplorerProps {
  range: TimeRange
}

const RANGE_TO_SQL_INTERVAL: Record<TimeRange, string> = {
  '15m': '15 minutes',
  '1h': '1 hour',
  '3h': '3 hours',
  '6h': '6 hours',
  '12h': '12 hours',
  '24h': '1 day',
  '7d': '7 days',
  '30d': '30 days',
}

function LogExplorer({ range }: LogExplorerProps) {
  const [logsFullscreen, setLogsFullscreen] = useState(false)
  const [shift, setShift] = useState(0)
  const [selectedService, setSelectedService] = useState('All services')
  // The typed-but-not-yet-run query; only applied to the filters below when
  // the user clicks "Run query" (or presses Enter) — no live search.
  const [draftSearch, setDraftSearch] = useState('')
  const [search, setSearch] = useState('')
  const [selectedFacets, setSelectedFacets] = useState<Record<string, string>>(
    {},
  )

  useEffect(() => {
    setShift(0)
  }, [range])

  const runQuery = () => {
    setSearch(draftSearch)
  }

  const filters: QueryFilters = useMemo(
    () => ({
      timeRange: RANGE_TO_SQL_INTERVAL[range],
      search: search || undefined,
      facets: {
        ...selectedFacets,
        ...(selectedService !== 'All services'
          ? { service: selectedService }
          : {}),
      },
    }),
    [range, search, selectedFacets, selectedService],
  )

  const { logs, histogram, facets } = useLogExplorer(filters, range)
  const activeSeverity =
    selectedFacets['severity'] ?? selectedFacets['level'] ?? extractSeverity(search)

  // Distinct services present in the current window, sourced from the facet
  // query so the dropdown always reflects what storage actually holds.
  const serviceOptions = useMemo(() => {
    const seen = new Set<string>()
    for (const row of facets ?? []) {
      const service = String(row.service ?? '').trim()
      if (service) seen.add(service)
    }
    return Array.from(seen).sort((a, b) => a.localeCompare(b))
  }, [facets])

  const handleFacetSelect = (queryAddition: string) => {
    const match = /^([\w ]+)='(.*)'$/.exec(queryAddition)
    if (!match) return
    const key = match[1].trim()
    const value = match[2]
    setSelectedFacets((prev) => {
      const next = { ...prev }
      if (next[key] === value) delete next[key]
      else next[key] = value
      return next
    })
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <section className="flex items-center gap-3 border-b border-zinc-700 bg-zinc-900 px-3 py-2">
        <ServiceSelect
          services={serviceOptions}
          value={selectedService}
          onChange={setSelectedService}
        />
        <SearchBar
          value={draftSearch}
          onChange={setDraftSearch}
          onSearch={runQuery}
        />
        <button
          type="button"
          onClick={runQuery}
          className="flex h-9 cursor-pointer items-center gap-1.5 rounded-md bg-[#a06bff] px-4 text-sm font-medium text-white transition-colors hover:bg-[#b18cff]"
        >
          <RiPlayFill className="h-4 w-4" />
          Run query
        </button>
      </section>
      <div className="flex min-h-0 flex-1">
        {!logsFullscreen && (
          <FiltersSidebar facets={facets} onFilterSelect={handleFacetSelect} />
        )}
        <main className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
          <Timeline
            fullscreen={logsFullscreen}
            range={range}
            shift={shift}
            histogram={histogram}
            severity={activeSeverity}
            onShiftChange={setShift}
          />
          <LogsList
            onToggleFullscreen={() => setLogsFullscreen((value) => !value)}
            range={range}
            shift={shift}
            logs={logs}
          />
        </main>
      </div>
    </div>
  )
}

export default LogExplorer