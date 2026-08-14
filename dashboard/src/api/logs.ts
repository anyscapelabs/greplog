const API_BASE = '/api'

export interface QueryFilters {
  timeRange: string // SQL interval string, e.g. '1 hour'
  search?: string
  facets: Record<string, string> // e.g. { severity: 'ERROR', service: 'api-gateway' }
}

const FACET_COLUMNS: Record<string, string> = {
  severity: 'level',
  service: 'service',
}

const FIELD_COLUMNS: Record<string, string> = {
  level: 'level',
  service: 'service',
  message: 'message',
  trace_id: 'trace_id',
  raw_body: 'raw_body',
}

const LOG_LEVELS = ['DEBUG', 'INFO', 'WARN', 'ERROR']

function escapeSql(value: string): string {
  return value.replace(/'/g, "''")
}

// Tokenizes an LQL query, honoring double-quoted values. e.g.
// `level=error "add to cart" trace_id:1-abc-123`.
function splitTerms(query: string): string[] {
  const terms: string[] = []
  const re = /"([^"]*)"|(\S+)/g
  let match: RegExpExecArray | null
  while ((match = re.exec(query)) !== null) {
    terms.push(match[1] ?? match[2])
  }
  return terms
}

function stripQuotes(value: string): string {
  return value.trim().replace(/^"(.*)"$/, '$1')
}

// Builds the SQL conditions for a single LQL term.
//
// Supported syntax:
// - `level=error` / `level:error`  -> case-insensitive level match
// - `service=api-gateway`          -> exact service match
// - `trace_id=1-abc-123`           -> exact trace id match
// - `message="add to cart"`        -> substring match on message
// - `raw_body=card_declined`       -> substring match on the JSON body
// - free text (e.g. `error`)       -> substring match across message, service
//   and raw_body, plus the level — so `error` finds ERROR logs.
function termConditions(term: string): string[] {
  const fieldMatch = /^([a-zA-Z_]+)\s*[=:]\s*(.+)$/.exec(term)
  if (fieldMatch) {
    const column = FIELD_COLUMNS[fieldMatch[1].toLowerCase()]
    const value = stripQuotes(fieldMatch[2])
    if (!column || !value) return []
    const escaped = escapeSql(value)
    if (column === 'message' || column === 'raw_body') {
      return [`${column} ILIKE '%${escaped}%'`]
    }
    if (column === 'level') {
      return [`LOWER(${column}) = '${escaped.toLowerCase()}'`]
    }
    return [`${column} = '${escaped}'`]
  }
  const escaped = escapeSql(stripQuotes(term))
  if (!escaped) return []
  return [
    `(message ILIKE '%${escaped}%' OR service ILIKE '%${escaped}%' OR raw_body ILIKE '%${escaped}%' OR LOWER(level) = '${escaped.toLowerCase()}')`,
  ]
}

/** Extracts an explicit `level=` value from an LQL query (e.g. "error" -> "ERROR"). */
export function extractSeverity(query: string): string | undefined {
  for (const term of splitTerms(query)) {
    const match = /^level\s*[=:]\s*(.+)$/i.exec(term)
    if (!match) continue
    const value = stripQuotes(match[1]).toUpperCase()
    if (LOG_LEVELS.includes(value)) return value
  }
  return undefined
}

// Builds the WHERE clause. Facet groups without a backing column in the
// Arrow schema (e.g. status, method) are skipped rather than failing the
// query. The search field is parsed as an LQL query.
function buildWhereClause(filters: QueryFilters): string {
  const conditions = [
    `timestamp_us >= (now() - INTERVAL '${escapeSql(filters.timeRange)}')`,
  ]

  Object.entries(filters.facets).forEach(([key, value]) => {
    const column = FACET_COLUMNS[key]
    if (column) {
      conditions.push(`${column} = '${escapeSql(value)}'`)
    }
  })

  if (filters.search) {
    for (const term of splitTerms(filters.search)) {
      conditions.push(...termConditions(term))
    }
  }

  return conditions.join(' AND ')
}

export interface QueryRow {
  [key: string]: unknown
}

async function postQuery(sql: string): Promise<QueryRow[]> {
  const response = await fetch(`${API_BASE}/query`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ sql }),
  })

  if (!response.ok) {
    throw new Error(`Greplog query failed (${response.status}): ${await response.text()}`)
  }

  return response.json()
}

export const logApi = {
  // 1. Fetch Log List
  fetchLogs: async (filters: QueryFilters): Promise<QueryRow[]> => {
    const where = buildWhereClause(filters)
    const sql = `SELECT timestamp_us, trace_id, level, service, message, raw_body FROM logs WHERE ${where} ORDER BY timestamp_us DESC LIMIT 500`
    return postQuery(sql)
  },

  // 2. Fetch Histogram Buckets
  fetchHistogram: async (
    filters: QueryFilters,
    intervalSecs: number,
  ): Promise<QueryRow[]> => {
    const where = buildWhereClause(filters)
    const sql = `
      SELECT date_bin(INTERVAL '${intervalSecs} seconds', timestamp_us, TIMESTAMP '1970-01-01T00:00:00.000000Z') AS bucket, COUNT(*) AS count
      FROM logs WHERE ${where}
      GROUP BY bucket ORDER BY bucket ASC
    `
    return postQuery(sql)
  },

  // 3. Fetch Sidebar Facets
  fetchFacets: async (filters: QueryFilters): Promise<QueryRow[]> => {
    // Only apply timeRange and search; facet selections must not filter the
    // facet counts themselves.
    const baseWhere = buildWhereClause({ ...filters, facets: {} })
    const sql = `
      SELECT level, service, COUNT(*) AS count
      FROM logs WHERE ${baseWhere}
      GROUP BY level, service
    `
    return postQuery(sql)
  },
}