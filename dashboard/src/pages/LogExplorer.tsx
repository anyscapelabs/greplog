import { useEffect, useMemo, useState } from 'react'
import { RiPlayFill } from 'react-icons/ri'
import SearchBar from '../components/SearchBar'
import ServiceSelect from '../components/ServiceSelect'
import ActiveFilterChips from '../components/logs/ActiveFilterChips'
import FiltersSidebar from '../components/logs/FiltersSidebar'
import LogsList from '../components/logs/LogsList'
import Timeline from '../components/logs/Timeline'
import { useLogExplorer } from '../hooks/useLogs'
import { extractSeverity } from '../api/logs'
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

  useEffect(() => {
    setTimelineShift(0)
  }, [range])

  const handleRunQuery = () => {
    setActiveSearchQuery(draftSearchQuery)
  }

  // Wire-named view of everything currently filtering the page: sidebar picks
  // plus the service dropdown. Drives both the chip bar and sidebar highlights.
  const effectiveFacets = useMemo(
    () => buildActiveFacets(selectedFacets, selectedService),
    [selectedFacets, selectedService],
  )

  const queryFilters = useMemo(() => {
    const timeRangeSecs = resolveTimeRangeSecs(range)
    if (!timeRangeSecs) return null

    return {
      timeRangeSecs,
      search: activeSearchQuery || undefined,
      facets: effectiveFacets,
    }
  }, [range, activeSearchQuery, effectiveFacets])

  const removeFacet = (key: string) => {
    // The service dropdown also injects a service facet; clearing only the
    // facet map would let the dropdown silently re-add it.
    if (key === 'service') setSelectedService('All services')
    setSelectedFacets((previous) => {
      const next = { ...previous }
      delete next[key]
      delete next[key === 'level' ? 'severity' : 'level']
      return next
    })
  }

  const clearAllFilters = () => {
    setSelectedService('All services')
    setSelectedFacets({})
    setActiveSearchQuery('')
    setDraftSearchQuery('')
  }

  const isFiltersValid = queryFilters !== null

  const { logs, histogram, facets, isLoading, isError, errorMessage } = useLogExplorer(
    queryFilters,
    range,
  )

  const discoveredServices = useMemo(() => getDiscoveredServices(facets as { service?: unknown }[] | undefined), [facets])

  const activeSeverity = useMemo(() => getActiveSeverity(selectedFacets, activeSearchQuery), [selectedFacets, activeSearchQuery])

  const handleFacetSelect = (facetQueryString: string) => {
    const parsed = parseFacetSelection(facetQueryString)

    if (!parsed) return

    const { facetKey, facetValue } = parsed

    setSelectedFacets((previousFacets) => {
      const nextFacets = { ...previousFacets }

      if (nextFacets[facetKey] === facetValue) {
        delete nextFacets[facetKey]
        return nextFacets
      }

      nextFacets[facetKey] = facetValue
      return nextFacets
    })
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
      <ActiveFilterChips
        facets={effectiveFacets}
        search={activeSearchQuery}
        onRemoveFacet={removeFacet}
        onRemoveSearch={() => {
          setActiveSearchQuery('')
          setDraftSearchQuery('')
        }}
        onClearAll={clearAllFilters}
      />
      {isError && (
        <div className="mx-3 mt-3 rounded-md border border-red-800 bg-red-950/40 px-3 py-2 text-sm text-red-300" role="alert">
          Failed to load logs: {errorMessage ?? 'Unknown error'}. Try adjusting filters or refreshing.
        </div>
      )}
      {isLoading && !isError && (
        <div className="mx-3 mt-3 rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-xs text-zinc-400">
          Loading logs…
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
          />
        </main>
      </div>
    </div>
  )
}

export default LogExplorer
