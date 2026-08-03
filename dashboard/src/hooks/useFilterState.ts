import { useCallback, useMemo } from 'react'
import { useSearchParams } from 'react-router-dom'

export type ChipPrefix = 'service' | 'route' | 'status' | 'correlation_id' | null

export interface FilterChip {
  prefix: ChipPrefix
  value: string
}

export interface FilterState {
  query: string
  chips: FilterChip[]
  services: string[]
  timeRange: string
  logLevels: string[]
  /** which items are checked per filter section, keyed by section id
   *  (e.g. `{ log_level: ['error'], response_status: ['server_error'] }`) */
  checked: Record<string, string[]>
}

const DEFAULT_TIME_RANGE = 'Last 15 min'

export const TIME_RANGE_NS: Record<string, number> = {
  'Last 15 min': 15 * 60 * 1_000_000_000,
  'Last 1 hour': 60 * 60 * 1_000_000_000,
  'Last 6 hours': 6 * 60 * 60 * 1_000_000_000,
  'Last 24 hours': 24 * 60 * 60 * 1_000_000_000,
  'Last 7 days': 7 * 24 * 60 * 60 * 1_000_000_000,
  'Last 30 days': 30 * 24 * 60 * 60 * 1_000_000_000,
}

function parseChips(raw: string | null): FilterChip[] {
  if (!raw) return []
  return raw.split(',').map((s) => {
    const trimmed = s.trim()
    if (!trimmed) return null
    const colonIdx = trimmed.indexOf(':')
    if (colonIdx > 0) {
      const prefix = trimmed.slice(0, colonIdx) as FilterChip['prefix']
      if (prefix === 'service' || prefix === 'route' || prefix === 'status' || prefix === 'correlation_id') {
        return { prefix, value: trimmed.slice(colonIdx + 1) }
      }
    }
    return { prefix: null, value: trimmed }
  }).filter((c): c is FilterChip => c !== null)
}

function encodeChips(chips: FilterChip[]): string {
  return chips.map((c) => (c.prefix ? `${c.prefix}:${c.value}` : c.value)).join(',')
}

function parseCommaList(raw: string | null): string[] {
  if (!raw) return []
  return raw.split(',').map((s) => s.trim()).filter(Boolean)
}

function parseChecked(raw: string | null): Record<string, string[]> {
  if (!raw) return {}
  try {
    const parsed = JSON.parse(raw) as Record<string, unknown>
    const result: Record<string, string[]> = {}
    for (const [sectionId, value] of Object.entries(parsed)) {
      if (Array.isArray(value)) {
        const ids = value.filter((v): v is string => typeof v === 'string' && v.length > 0)
        if (ids.length > 0) result[sectionId] = ids
      }
    }
    return result
  } catch {
    return {}
  }
}

function encodeChecked(checked: Record<string, string[]>): string {
  return JSON.stringify(checked)
}

function filterStateFromParams(sp: URLSearchParams): FilterState {
  const chips = parseChips(sp.get('c'))
  const services = parseCommaList(sp.get('s'))
  const chipServices = chips
    .filter((ch): ch is FilterChip & { prefix: 'service' } => ch.prefix === 'service')
    .map((ch) => ch.value)
  const mergedServices = [...new Set([...services, ...chipServices])]
  return {
    query: sp.get('q') ?? '',
    chips,
    services: mergedServices,
    timeRange: sp.get('t') ?? DEFAULT_TIME_RANGE,
    logLevels: parseCommaList(sp.get('l')),
    checked: parseChecked(sp.get('ch')),
  }
}

export function useFilterState() {
  const [searchParams, setSearchParams] = useSearchParams()

  const filters = useMemo(() => filterStateFromParams(searchParams), [searchParams])

  const apply = useCallback((updater: (prev: URLSearchParams) => void) => {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev)
      updater(next)
      return next
    }, { replace: true })
  }, [setSearchParams])

  const setQuery = useCallback((query: string) => {
    apply((p) => {
      if (query) p.set('q', query)
      else p.delete('q')
    })
  }, [apply])

  const addChip = useCallback((chip: FilterChip) => {
    apply((p) => {
      const chips = parseChips(p.get('c'))
      chips.push(chip)
      p.set('c', encodeChips(chips))
      if (chip.prefix === 'service') {
        const svc = parseCommaList(p.get('s'))
        if (!svc.includes(chip.value)) {
          svc.push(chip.value)
          p.set('s', svc.join(','))
        }
      }
    })
  }, [apply])

  const removeChip = useCallback((chip: FilterChip) => {
    apply((p) => {
      const chips = parseChips(p.get('c'))
      const idx = chips.findIndex(
        (c) => c.prefix === chip.prefix && c.value === chip.value,
      )
      if (idx >= 0) {
        chips.splice(idx, 1)
        p.set('c', encodeChips(chips))
      }
      if (chip.prefix === 'service') {
        const svc = parseCommaList(p.get('s'))
        const filtered = svc.filter((s) => s !== chip.value)
        if (filtered.length > 0) p.set('s', filtered.join(','))
        else p.delete('s')
      }
      if (chips.length === 0) p.delete('c')
    })
  }, [apply])

  const toggleService = useCallback((service: string) => {
    apply((p) => {
      const svc = parseCommaList(p.get('s'))
      const idx = svc.indexOf(service)
      if (idx >= 0) {
        svc.splice(idx, 1)
      } else {
        svc.push(service)
      }
      if (svc.length > 0) p.set('s', svc.join(','))
      else p.delete('s')
    })
  }, [apply])

  const setServices = useCallback((services: string[]) => {
    apply((p) => {
      if (services.length > 0) p.set('s', services.join(','))
      else p.delete('s')
    })
  }, [apply])

  const setTimeRange = useCallback((timeRange: string) => {
    apply((p) => {
      p.set('t', timeRange)
    })
  }, [apply])

  const setLogLevels = useCallback((levels: string[]) => {
    apply((p) => {
      if (levels.length > 0) p.set('l', levels.join(','))
      else p.delete('l')
    })
  }, [apply])

  const toggleChecked = useCallback((sectionId: string, id: string) => {
    apply((p) => {
      const checked = parseChecked(p.get('ch'))
      const ids = checked[sectionId] ?? []
      const idx = ids.indexOf(id)
      if (idx >= 0) {
        ids.splice(idx, 1)
      } else {
        ids.push(id)
      }
      if (ids.length > 0) checked[sectionId] = ids
      else delete checked[sectionId]
      if (Object.keys(checked).length > 0) p.set('ch', encodeChecked(checked))
      else p.delete('ch')
    })
  }, [apply])

  const setChecked = useCallback((sectionId: string, id: string, value: boolean) => {
    apply((p) => {
      const checked = parseChecked(p.get('ch'))
      const ids = checked[sectionId] ?? []
      const idx = ids.indexOf(id)
      if (value) {
        if (idx < 0) ids.push(id)
        checked[sectionId] = ids
      } else if (idx >= 0) {
        ids.splice(idx, 1)
        if (ids.length > 0) checked[sectionId] = ids
        else delete checked[sectionId]
      }
      if (Object.keys(checked).length > 0) p.set('ch', encodeChecked(checked))
      else p.delete('ch')
    })
  }, [apply])

  const clearAll = useCallback(() => {
    apply((p) => {
      p.delete('q')
      p.delete('c')
      p.delete('s')
      p.delete('l')
      p.delete('ch')
    })
  }, [apply])

  const clearFilters = useCallback(() => {
    apply((p) => {
      p.delete('c')
      p.delete('s')
      p.delete('l')
      p.delete('ch')
    })
  }, [apply])

  return {
    filters,
    setQuery,
    addChip,
    removeChip,
    toggleService,
    setServices,
    setTimeRange,
    setLogLevels,
    toggleChecked,
    setChecked,
    clearAll,
    clearFilters,
  }
}

const PREFIX_PATTERN = /^(service|route|status|correlation_id):(.+)$/

export function parseQueryToChip(raw: string): FilterChip {
  const match = raw.match(PREFIX_PATTERN)
  if (match) {
    const prefix = match[1] as FilterChip['prefix']
    return { prefix, value: match[2] }
  }
  return { prefix: null, value: raw }
}

export function chipDisplay(chip: FilterChip): string {
  return chip.prefix ? `${chip.prefix}:${chip.value}` : chip.value
}

function chipToClause(chip: FilterChip, includeService: boolean): string | null {
  if (chip.prefix === 'service') {
    if (!includeService) return null
    return `service = '${chip.value.replace(/'/g, "''")}'`
  }
  if (chip.prefix === 'correlation_id') {
    return `correlation_id = '${chip.value.replace(/'/g, "''")}'`
  }
  if (chip.prefix === 'route') {
    return `message LIKE '%${chip.value.replace(/'/g, "''")}%'`
  }
  if (chip.prefix === 'status') {
    const match = chip.value.match(/^([><=!]+)(\d+)$/)
    if (match) {
      return `line ${match[1]} ${match[2]}`
    }
    return `line = '${chip.value.replace(/'/g, "''")}'`
  }
  const escaped = chip.value.replace(/'/g, "''")
  return `(message LIKE '%${escaped}%' OR level = '${escaped}')`
}

function quoteSqlList(values: string[]): string {
  return values.map((v) => `'${v.replace(/'/g, "''")}'`).join(',')
}

// Translate checked sidebar items into SQL. Each filter section maps to a
// distinct predicate; sections that only narrow the client side (e.g. the
// Services page's health_status) intentionally compile to nothing.
export function compileCheckedToQuery(checked: Record<string, string[]>): string {
  const clauses: string[] = []
  for (const [sectionId, ids] of Object.entries(checked)) {
    if (!ids || ids.length === 0) continue
    let clause: string | null = null
    switch (sectionId) {
      case 'log_level': {
        const values = ids.filter((v) => v.trim())
        if (values.length > 0) clause = `level IN (${quoteSqlList(values)})`
        break
      }
      case 'service_name': {
        const values = ids.filter((v) => v.trim())
        if (values.length > 0) clause = `service IN (${quoteSqlList(values)})`
        break
      }
      case 'status_code': {
        const nums = ids.map(Number).filter((n) => Number.isFinite(n))
        if (nums.length > 0) clause = `line IN (${nums.join(',')})`
        break
      }
      case 'response_status': {
        const ranges: string[] = []
        if (ids.includes('success')) ranges.push('line < 300')
        if (ids.includes('redirect')) ranges.push('line >= 300 AND line < 400')
        if (ids.includes('client_error')) ranges.push('line >= 400 AND line < 500')
        if (ids.includes('server_error')) ranges.push('line >= 500')
        if (ranges.length > 0) clause = `(${ranges.join(' OR ')})`
        break
      }
      case 'error_type': {
        const values = ids.filter((v) => v.trim())
        if (values.length > 0) clause = `exception_type IN (${quoteSqlList(values)})`
        break
      }
      default:
        // Unknown / client-side-only sections (health_status, future ones)
        // are not compiled to SQL.
        clause = null
    }
    if (clause) clauses.push(clause)
  }
  return clauses.join(' AND ')
}

// Compile filter state into a SQL WHERE clause. `liveQuery` is the raw text
// currently being typed in the search box (before it is committed as a chip):
// it is compiled the same way a chip would be so the chart and log queries
// narrow as the user types, without waiting for Enter.
export function compileFilterToQuery(filters: FilterState, liveQuery?: string): string {
  const clauses: string[] = []

  if (filters.services.length > 0) {
    const quoted = filters.services.map((s) => `'${s.replace(/'/g, "''")}'`).join(',')
    clauses.push(`service IN (${quoted})`)
  }

  if (filters.logLevels.length > 0) {
    const quoted = filters.logLevels.map((l) => `'${l.replace(/'/g, "''")}'`).join(',')
    clauses.push(`level IN (${quoted})`)
  }

  for (const chip of filters.chips) {
    const clause = chipToClause(chip, false)
    if (clause) clauses.push(clause)
  }

  const trimmedLive = liveQuery?.trim()
  if (trimmedLive) {
    const liveChip = parseQueryToChip(trimmedLive)
    const alreadyPinned = filters.chips.some(
      (c) => c.prefix === liveChip.prefix && c.value === liveChip.value,
    )
    if (!alreadyPinned) {
      const clause = chipToClause(liveChip, true)
      if (clause) clauses.push(clause)
    }
  }

  const checkedClause = compileCheckedToQuery(filters.checked)
  if (checkedClause) clauses.push(checkedClause)

  const windowNs = TIME_RANGE_NS[filters.timeRange]
  if (windowNs && windowNs > 0) {
    const nowMicros = Date.now() * 1_000
    clauses.push(`timestamp > to_timestamp_micros(${nowMicros - windowNs / 1_000})`)
  }

  if (clauses.length === 0) return ''
  return 'WHERE ' + clauses.join(' AND ')
}
