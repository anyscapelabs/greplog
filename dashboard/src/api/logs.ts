const API_BASE = '/api'

export interface QueryFilters {
  timeRangeSecs: number
  search?: string
  facets: Record<string, string>
}

const LOG_LEVELS = ['DEBUG', 'INFO', 'WARN', 'ERROR']

/** Extracts an explicit `level=` value from an LQL query (e.g. "error" -> "ERROR"). */
export function extractSeverity(query: string): string | undefined {
  if (!query) return undefined

  for (const term of splitTerms(query)) {
    const levelMatch = /^level\s*[=:]\s*(.+)$/i.exec(term)
    if (!levelMatch) continue

    const normalizedLevel = stripQuotes(levelMatch[1]).toUpperCase()
    if (!LOG_LEVELS.includes(normalizedLevel)) continue

    return normalizedLevel
  }

  return undefined
}

function splitTerms(query: string): string[] {
  const trimmedQuery = query.trim()
  if (!trimmedQuery) return []

  const terms: string[] = []
  const termPattern = /"([^"]*)"|(\S+)/g
  let match: RegExpExecArray | null

  while ((match = termPattern.exec(trimmedQuery)) !== null) {
    const term = match[1] ?? match[2]
    if (term !== undefined) terms.push(term)
  }

  return terms
}

function stripQuotes(value: string): string {
  if (!value) return ''
  return value.trim().replace(/^"(.*)"$/, '$1')
}

type SearchFacets = Record<string, string>

interface RowsSearch {
  type: 'rows'
  time_range_secs: number
  facets: SearchFacets
  search?: string
  limit: number
}

interface AggregateSearch {
  type: 'aggregate'
  time_range_secs: number
  facets: SearchFacets
  search?: string
  group_by: string[]
  bucket_secs?: number
  metrics: string[]
}

type SearchRequest = RowsSearch | AggregateSearch

function validateFilters(filters: QueryFilters): void {
  if (!filters) throw new Error('Query filters are required')
  if (!Number.isFinite(filters.timeRangeSecs) || filters.timeRangeSecs <= 0) {
    throw new Error('filters.timeRangeSecs must be a positive number')
  }
}

/** The UI says "severity"; the server's column is "level". Unknown facet
 * keys are dropped here rather than surfacing as 400s from the server. */
const FACET_WIRE_NAMES: Record<string, string> = {
  severity: 'level',
  level: 'level',
  service: 'service',
}

function toWireFacets(facets: Record<string, string> | undefined): SearchFacets {
  const translated: SearchFacets = {}
  for (const [key, value] of Object.entries(facets ?? {})) {
    const wireName = FACET_WIRE_NAMES[key]
    if (wireName && value.trim()) translated[wireName] = value
  }
  return translated
}

async function postSearch(request: SearchRequest): Promise<QueryRow[]> {
  const response = await fetch(`${API_BASE}/search`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request),
  })

  if (!response.ok) {
    throw new Error(await describeError(response))
  }

  const payload = (await response.json()) as QueryRow[]
  if (!Array.isArray(payload)) throw new Error('Unexpected search response: expected an array of rows')

  return payload
}

/** Surfaces the server's error message; falls back to the status text. */
async function describeError(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { message?: string }
    if (body?.message) return `${body.message} (${response.status})`
  } catch {
    // not JSON; fall through
  }
  return `Greplog query failed (${response.status})`
}

export interface QueryRow {
  [key: string]: unknown
}

export interface StorageStats {
  bytes: number
  partitions: number
  chunks: number
}

export const logApi = {
  fetchLogs: async (filters: QueryFilters): Promise<QueryRow[]> => {
    validateFilters(filters)

    return postSearch({
      type: 'rows',
      time_range_secs: filters.timeRangeSecs,
      facets: toWireFacets(filters.facets),
      search: filters.search || undefined,
      limit: 500,
    })
  },

  fetchHistogram: async (
    filters: QueryFilters,
    intervalSecs: number,
  ): Promise<QueryRow[]> => {
    validateFilters(filters)

    return postSearch({
      type: 'aggregate',
      time_range_secs: filters.timeRangeSecs,
      facets: toWireFacets(filters.facets),
      search: filters.search || undefined,
      group_by: ['bucket'],
      bucket_secs: intervalSecs,
      metrics: ['count'],
    })
  },

  fetchFacets: async (filters: QueryFilters): Promise<QueryRow[]> => {
    validateFilters(filters)

    // Facets always show the full window, independent of the active facet picks.
    return postSearch({
      type: 'aggregate',
      time_range_secs: filters.timeRangeSecs,
      facets: {},
      group_by: ['level', 'service'],
      metrics: ['count'],
    })
  },

  fetchIngestion: async (
    timeRangeSecs: number,
    intervalSecs: number,
  ): Promise<QueryRow[]> => {
    if (!Number.isFinite(timeRangeSecs) || timeRangeSecs <= 0) {
      throw new Error('timeRangeSecs must be a positive number')
    }

    return postSearch({
      type: 'aggregate',
      time_range_secs: timeRangeSecs,
      facets: {},
      group_by: ['bucket'],
      bucket_secs: intervalSecs,
      metrics: ['count'],
    })
  },

  fetchSeverityBreakdown: async (
    filters: QueryFilters,
    intervalSecs: number,
  ): Promise<QueryRow[]> => {
    validateFilters(filters)

    return postSearch({
      type: 'aggregate',
      time_range_secs: filters.timeRangeSecs,
      facets: toWireFacets(filters.facets),
      search: filters.search || undefined,
      group_by: ['bucket', 'level'],
      bucket_secs: intervalSecs,
      metrics: ['count'],
    })
  },

  fetchIngestionByService: async (
    filters: QueryFilters,
    intervalSecs: number,
  ): Promise<QueryRow[]> => {
    validateFilters(filters)

    return postSearch({
      type: 'aggregate',
      time_range_secs: filters.timeRangeSecs,
      facets: toWireFacets(filters.facets),
      search: filters.search || undefined,
      group_by: ['bucket', 'service'],
      bucket_secs: intervalSecs,
      metrics: ['count'],
    })
  },

  fetchServiceTable: async (filters: QueryFilters): Promise<QueryRow[]> => {
    validateFilters(filters)

    return postSearch({
      type: 'aggregate',
      time_range_secs: filters.timeRangeSecs,
      facets: toWireFacets(filters.facets),
      search: filters.search || undefined,
      group_by: ['service'],
      metrics: ['count', 'errors', 'warns', 'last_seen'],
    })
  },

  /** Overall error percentage over the window: one global aggregate row. */
  fetchErrorRate: async (filters: QueryFilters): Promise<QueryRow[]> => {
    validateFilters(filters)

    return postSearch({
      type: 'aggregate',
      time_range_secs: filters.timeRangeSecs,
      facets: toWireFacets(filters.facets),
      search: filters.search || undefined,
      group_by: [],
      metrics: ['count', 'errors'],
    })
  },

  fetchStorage: async (): Promise<StorageStats> => {
    const response = await fetch(`${API_BASE}/stats`)
    if (!response.ok) throw new Error(await describeError(response))

    const stats = (await response.json()) as StorageStats
    if (!Number.isFinite(stats.bytes)) throw new Error('Unexpected stats response')
    return stats
  },
}
