import { toastStore } from '../lib/toastStore.ts'

const API_BASE = import.meta.env.VITE_API_BASE ?? ''

export interface QueryResult {
  columns: string[]
  rows: unknown[][]
  row_count: number
}

export interface HealthResponse {
  status: string
  version: string
}

export async function fetchHealth(): Promise<HealthResponse | null> {
  try {
    const res = await fetch(`${API_BASE}/health`, { signal: AbortSignal.timeout(3000) })
    if (!res.ok) {
      const body = await res.text().catch(() => '(unreadable)')
      console.error(`[api] /health failed: ${res.status} ${res.statusText} — ${body}`)
      return null
    }
    return (await res.json()) as HealthResponse
  } catch (err) {
    console.error('[api] /health error:', err)
    return null
  }
}

export interface DetectEntry {
  service_name: string | null
  language: string
  framework: string | null
  project_file: string
}

export async function fetchDetect(): Promise<DetectEntry[] | null> {
  try {
    const res = await fetch(`${API_BASE}/detect`, { signal: AbortSignal.timeout(5000) })
    if (!res.ok) {
      const body = await res.text().catch(() => '(unreadable)')
      console.error(`[api] /detect failed: ${res.status} ${res.statusText} — ${body}`)
      return null
    }
    return (await res.json()) as DetectEntry[]
  } catch (err) {
    console.error('[api] /detect error:', err)
    return null
  }
}

export async function postQuery(sql: string): Promise<QueryResult | null> {
  const endpoint = '/query'
  try {
    const res = await fetch(`${API_BASE}/query`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ sql }),
      signal: AbortSignal.timeout(30_000),
    })
    if (!res.ok) {
      const body = await res.text().catch(() => '(unreadable)')
      console.error(`[api] /query failed (${res.status}): SQL="${sql.slice(0, 200)}" — ${body}`)
      toastStore.showError('Query failed — showing no data', { dedupeKey: `query-error:${endpoint}` })
      return null
    }
    const data = (await res.json()) as QueryResult
    toastStore.showSuccess('Query succeeded again', { dedupeKey: `query-error:${endpoint}` })
    return data
  } catch (err) {
    console.error('[api] /query error:', err)
    toastStore.showError('Query failed — showing no data', { dedupeKey: `query-error:${endpoint}` })
    return null
  }
}
