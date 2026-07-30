const API_BASE = import.meta.env.VITE_API_BASE ?? ''

export interface QueryResult {
  columns: string[]
  rows: string[][]
  row_count: number
}

export interface HealthResponse {
  status: string
  version: string
}

export async function fetchHealth(): Promise<HealthResponse | null> {
  try {
    const res = await fetch(`${API_BASE}/health`, { signal: AbortSignal.timeout(3000) })
    if (!res.ok) return null
    return (await res.json()) as HealthResponse
  } catch {
    return null
  }
}

export async function postQuery(sql: string): Promise<QueryResult | null> {
  try {
    const res = await fetch(`${API_BASE}/query`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ sql }),
      signal: AbortSignal.timeout(30_000),
    })
    if (!res.ok) return null
    return (await res.json()) as QueryResult
  } catch {
    return null
  }
}
