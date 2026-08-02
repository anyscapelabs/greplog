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
  checked: Record<string, boolean>
}

const DEFAULT_TIME_RANGE = 'Last 15 min'

export const TIME_RANGE_NS: Record<string, number> = {
  'Last 15 min': 15 * 60 * 1_000_000_000,
  'Last 1 hour': 60 * 60 * 1_000_000_000,
  'Last 6 hours': 6 * 60 * 60 * 1_000_000_000,
  'Last 24 hours': 24 * 60 * 60 * 1_000_000_000,
  'Last 7 days': 7 * 24 * 60 * 60 * 1_000_000_000,
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

function parseChecked(raw: string | null): Record<string, boolean> {
  if (!raw) return {}
  try {
    return JSON.parse(raw) as Record<string, boolean>
  } catch {
    return {}
  }
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

  const toggleChecked = useCallback((id: string) => {
    apply((p) => {
      const checked = parseChecked(p.get('ch'))
      checked[id] = !checked[id]
      p.set('ch', JSON.stringify(checked))
    })
  }, [apply])

  const setChecked = useCallback((id: string, value: boolean) => {
    apply((p) => {
      const checked = parseChecked(p.get('ch'))
      if (value) checked[id] = true
      else delete checked[id]
      const keys = Object.keys(checked)
      if (keys.length > 0) p.set('ch', JSON.stringify(checked))
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

export function compileFilterToQuery(filters: FilterState): string {
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
    if (chip.prefix === 'service') continue
    if (chip.prefix === 'correlation_id') {
      clauses.push(`correlation_id = '${chip.value.replace(/'/g, "''")}'`)
      continue
    }
    if (chip.prefix === 'route') {
      clauses.push(`message LIKE '%${chip.value.replace(/'/g, "''")}%'`)
    } else if (chip.prefix === 'status') {
      const match = chip.value.match(/^([><=!]+)(\d+)$/)
      if (match) {
        clauses.push(`line ${match[1]} ${match[2]}`)
      } else {
        clauses.push(`line = '${chip.value.replace(/'/g, "''")}'`)
      }
    } else {
      clauses.push(`message LIKE '%${chip.value.replace(/'/g, "''")}%'`)
    }
  }

  const windowNs = TIME_RANGE_NS[filters.timeRange]
  if (windowNs && windowNs > 0) {
    const nowMicros = Date.now() * 1_000
    clauses.push(`timestamp > to_timestamp_micros(${nowMicros - windowNs / 1_000})`)
  }

  if (clauses.length === 0) return ''
  return 'WHERE ' + clauses.join(' AND ')
}
