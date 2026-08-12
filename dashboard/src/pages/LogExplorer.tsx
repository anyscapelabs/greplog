import { useEffect, useMemo, useState } from 'react'
import { RiPlayFill } from 'react-icons/ri'
import SearchBar from '../components/SearchBar'
import ServiceSelect from '../components/ServiceSelect'
import FiltersSidebar from '../components/logs/FiltersSidebar'
import LogsList from '../components/logs/LogsList'
import Timeline from '../components/logs/Timeline'
import { useLogExplorer } from '../hooks/useLogs'
import type { QueryFilters } from '../api/logs'
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
  const [search, setSearch] = useState('')
  const [selectedFacets, setSelectedFacets] = useState<Record<string, string>>(
    {},
  )

  useEffect(() => {
    setShift(0)
  }, [range])

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

  const { logs, histogram, facets } = useLogExplorer(filters)

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
        <ServiceSelect value={selectedService} onChange={setSelectedService} />
        <SearchBar value={search} onChange={setSearch} />
        <button
          type="button"
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