import { useMemo, useState } from 'react'
import FiltersSidebar from '../components/logs/FiltersSidebar'
import Timeline from '../components/logs/Timeline'
import { RANGE_SECONDS } from '../components/logs/Timeline'
import { useErrorRate, useLogExplorer, useIngestion, useStorage } from '../hooks/useLogs'
import type { QueryFilters } from '../api/logs'
import type { TimeRange } from '../components/Header'
import SeverityBreakdown from '../components/metrics/SeverityBreakdown'
import ErrorRate from '../components/metrics/ErrorRate'
import Storage from '../components/metrics/Storage'
import IngestionByService from '../components/metrics/IngestionByService'
import ServiceTable from '../components/metrics/ServiceTable'

interface MetricsProps {
  range: TimeRange
}

function Metrics({ range }: MetricsProps) {
  const [selectedFacets, setSelectedFacets] = useState<Record<string, string>>(
    {},
  )
  const [shift, setShift] = useState(0)

  const filters: QueryFilters = useMemo(
    () => ({
      timeRangeSecs: RANGE_SECONDS[range],
      facets: selectedFacets,
    }),
    [range, selectedFacets],
  )

  const { facets } = useLogExplorer(filters, range)
  const { data: ingestion } = useIngestion(range)
  const errorRate = useErrorRate(filters)
  const storage = useStorage()

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
    <div className="flex min-h-0 flex-1">
      <FiltersSidebar
        facets={facets}
        active={{
          level: selectedFacets.level ?? selectedFacets.severity,
          service: selectedFacets.service,
        }}
        onFilterSelect={handleFacetSelect}
      />
      <main className="flex min-w-0 flex-1 flex-col overflow-y-auto">
        <Timeline
          fullscreen={false}
          range={range}
          shift={shift}
          histogram={ingestion ?? []}
          title="Ingestion"
          onShiftChange={setShift}
        />
        <div className="flex gap-3 p-3">
          <SeverityBreakdown range={range} filters={filters} shift={shift} />
          <div className="flex h-[350px] min-h-[350px] w-[32%] flex-col gap-3">
            <ErrorRate
              value={errorRate.value}
              isLoading={errorRate.isLoading}
              isError={errorRate.isError}
              errorMessage={errorRate.errorMessage}
            />
            <Storage
              bytes={storage.bytes}
              isLoading={storage.isLoading}
              isError={storage.isError}
              errorMessage={storage.errorMessage}
            />
          </div>
        </div>
        <div className="p-3 pt-0">
          <IngestionByService range={range} filters={filters} shift={shift} />
        </div>
        <div className="p-3 pt-0">
          <ServiceTable range={range} filters={filters} />
        </div>
      </main>
    </div>
  )
}

export default Metrics