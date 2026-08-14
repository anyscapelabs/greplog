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

function escapeSql(value: string): string {
  return value.replace(/'/g, "''")
}

// Builds the WHERE clause. Facet groups without a backing column in the
// Arrow schema (e.g. status, method) are skipped rather than failing the
// query.
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
    conditions.push(`message ILIKE '%${escapeSql(filters.search)}%'`)
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
    interval: string = '1 minute',
  ): Promise<QueryRow[]> => {
    const where = buildWhereClause(filters)
    const sql = `
      SELECT date_bin(INTERVAL '${escapeSql(interval)}', timestamp_us, TIMESTAMP '1970-01-01T00:00:00.000000Z') AS bucket, COUNT(*) AS count
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