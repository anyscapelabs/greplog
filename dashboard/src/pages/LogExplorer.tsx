import { useEffect, useMemo, useState } from 'react'
import { RiPlayFill } from 'react-icons/ri'
import SearchBar from '../components/SearchBar'
import ServiceSelect from '../components/ServiceSelect'
import FiltersSidebar from '../components/logs/FiltersSidebar'
import LogsList from '../components/logs/LogsList'
import Timeline from '../components/logs/Timeline'
import { useLogExplorer } from '../hooks/useLogs'
import { ROWS_PAGE_SIZE, extractSeverity } from '../api/logs'
import { RANGE_SECONDS } from '../components/logs/Timeline'
import type { TimeRange } from '../components/Header'

interface LogExplorerProps {
  range: TimeRange
  liveTailActive?: boolean
}

function buildActiveFacets(
  selectedFacets: Record<string, string>,
  selectedService: string,
): Record<string, string> {
  if (selectedService === 'All services') return { ...selectedFacets }

  return { ...selectedFacets, service: selectedService }
}

function resolveTimeRangeSecs(range: TimeRange): number | null {
  if (!range) return null

  return RANGE_SECONDS[range] ?? null
}

function getDiscoveredServices(facets: { service?: unknown }[] | undefined): string[] {
  if (!facets) return []

  const discoveredServices = new Set<string>()

  for (const facetRow of facets) {
    const rawService = String(facetRow.service ?? '').trim()

    if (!rawService) continue

    discoveredServices.add(rawService)
  }

  return Array.from(discoveredServices).sort((first, second) => first.localeCompare(second))
}

function getActiveSeverity(
  selectedFacets: Record<string, string>,
  activeSearchQuery: string,
): string | undefined {
  const facetSeverity = selectedFacets['severity'] ?? selectedFacets['level']

  if (facetSeverity) return facetSeverity

  return extractSeverity(activeSearchQuery)
}

function parseFacetSelection(facetQueryString: string): { facetKey: string; facetValue: string } | null {
  if (!facetQueryString) return null

  const trimmedQuery = facetQueryString.trim()

  if (!trimmedQuery) return null

  const facetMatch = /^([\w ]+)='(.*)'$/.exec(trimmedQuery)

  if (!facetMatch) return null

  const facetKey = facetMatch[1].trim()

  if (!facetKey) return null

  const facetValue = facetMatch[2]

  return { facetKey, facetValue }
}

function LogExplorer({ range, liveTailActive: _liveTailActive = false }: LogExplorerProps) {
  const [isLogsFullscreen, setIsLogsFullscreen] = useState(false)
  const [timelineShift, setTimelineShift] = useState(0)
  const [selectedService, setSelectedService] = useState('All services')
  const [draftSearchQuery, setDraftSearchQuery] = useState('')
  const [activeSearchQuery, setActiveSearchQuery] = useState('')
  const [selectedFacets, setSelectedFacets] = useState<Record<string, string>>(
    {},
  )
  const [page, setPage] = useState(0)

  // Wire-named view of everything currently filtering the page: sidebar picks
  // plus the service dropdown. Drives both the chip bar and sidebar highlights.
  const effectiveFacets = useMemo(
    () => buildActiveFacets(selectedFacets, selectedService),
    [selectedFacets, selectedService],
  )

  useEffect(() => {
    setTimelineShift(0)
  }, [range])

  // Reset pagination whenever filters change
  useEffect(() => {
    setPage(0)
  }, [range, activeSearchQuery, effectiveFacets])

  const handleRunQuery = () => {
    setActiveSearchQuery(draftSearchQuery)
    setPage(0)
  }

  const queryFilters = useMemo(() => {
    const timeRangeSecs = resolveTimeRangeSecs(range)
    if (!timeRangeSecs) return null

    return {
      timeRangeSecs,
      search: activeSearchQuery || undefined,
      facets: effectiveFacets,
    }
  }, [range, activeSearchQuery, effectiveFacets])

  const isFiltersValid = queryFilters !== null
  const offset = page * ROWS_PAGE_SIZE

  const { logs, histogram, facets, isLoading, isError, errorMessage, refetchLogs } = useLogExplorer(
    queryFilters,
    range,
    offset,
  )

  const discoveredServices = useMemo(() => getDiscoveredServices(facets as { service?: unknown }[] | undefined), [facets])

  const activeSeverity = useMemo(() => getActiveSeverity(selectedFacets, activeSearchQuery), [selectedFacets, activeSearchQuery])

  const handleFacetSelect = (queryAddition: string) => {
    const parsed = parseFacetSelection(queryAddition)

    if (!parsed) return

    const { facetKey, facetValue } = parsed
    const next = { ...selectedFacets }

    if (next[facetKey] === facetValue) {
      // Toggle off. The service dropdown also injects a service facet;
      // clearing only the map would let the dropdown silently re-add it.
      delete next[facetKey]
      if (facetKey === 'service' && selectedService === facetValue) {
        setSelectedService('All services')
      }
      setSelectedFacets(next)
      return
    }

    next[facetKey] = facetValue
    setSelectedFacets(next)
  }

  if (!range) {
    return (
      <div className="flex flex-1 items-center justify-center p-8 text-sm text-red-400">
        Invalid time range: expected a valid TimeRange value.
      </div>
    )
  }

  if (!isFiltersValid) {
    return (
      <div className="flex flex-1 items-center justify-center p-8 text-sm text-red-400">
        Invalid query filters: unsupported time range &ldquo;{range}&rdquo;.
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <section className="flex items-center gap-3 border-b border-zinc-700 bg-zinc-900 px-3 py-2">
        <ServiceSelect
          services={discoveredServices}
          value={selectedService}
          onChange={setSelectedService}
        />
        <SearchBar
          value={draftSearchQuery}
          onChange={setDraftSearchQuery}
          onSearch={handleRunQuery}
        />
        <button
          type="button"
          onClick={handleRunQuery}
          className="flex h-9 cursor-pointer items-center gap-1.5 rounded-md bg-[#a06bff] px-4 text-sm font-medium text-white transition-colors hover:bg-[#b18cff]"
        >
          <RiPlayFill className="h-4 w-4" />
          Run query
        </button>
      </section>
      {isError && (
        <div className="mx-3 mt-3 rounded-md border border-red-800 bg-red-950/40 px-3 py-2 text-sm text-red-300" role="alert">
          Failed to load logs: {errorMessage ?? 'Unknown error'}. Try adjusting filters or refreshing.
        </div>
      )}
      <div className="flex min-h-0 flex-1">
        {!isLogsFullscreen && (
          <FiltersSidebar
            facets={facets}
            active={{
              level: effectiveFacets.level ?? effectiveFacets.severity,
              service: effectiveFacets.service,
            }}
            onFilterSelect={handleFacetSelect}
          />
        )}
        <main className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
          <Timeline
            fullscreen={isLogsFullscreen}
            range={range}
            shift={timelineShift}
            histogram={histogram}
            severity={activeSeverity}
            onShiftChange={setTimelineShift}
          />
          <LogsList
            onToggleFullscreen={() => setIsLogsFullscreen((previousValue) => !previousValue)}
            range={range}
            shift={timelineShift}
            logs={logs}
            isLoading={isLoading && !isError}
            page={page}
            pageSize={ROWS_PAGE_SIZE}
            onPageChange={setPage}
            onRefresh={() => refetchLogs()}
          />
        </main>
      </div>
    </div>
  )
}

export default LogExplorer
