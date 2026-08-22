import { useEffect, useRef, useState } from 'react'
import { LuPause, LuPlay, LuTrash2 } from 'react-icons/lu'
import type { QueryRow } from '../api/logs'

/** Newest entries kept in memory; older ones fall off the top. */
const MAX_TAIL_ENTRIES = 500

type Connection = 'connecting' | 'live' | 'closed'

interface LogEntry {
  kind: 'log'
  id: number
  timestampUs: unknown
  level: string
  service: string
  message: string
}

interface GapEntry {
  kind: 'gap'
  id: number
  skipped: number
}

type TailEntry = LogEntry | GapEntry

const LEVEL_STYLES: Record<string, string> = {
  DEBUG: 'text-zinc-500',
  INFO: 'text-sky-400',
  WARN: 'text-amber-400',
  ERROR: 'text-red-400',
  FATAL: 'text-red-400',
  CRITICAL: 'text-red-400',
}

const CONNECTION_STYLES: Record<Connection, { dot: string; label: string }> = {
  connecting: { dot: 'bg-amber-400', label: 'connecting…' },
  live: { dot: 'bg-emerald-400', label: 'live' },
  closed: { dot: 'bg-zinc-600', label: 'disconnected' },
}

function formatTimestamp(timestampUs: unknown): string {
  const micros =
    typeof timestampUs === 'number'
      ? timestampUs
      : Number(timestampUs) || Date.parse(`${String(timestampUs)}Z`) * 1000
  if (!Number.isFinite(micros)) return ''

  const date = new Date(micros / 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}

/**
 * Live log stream over `GET /api/tail` (SSE). Each `logs` event carries one
 * broadcast batch; a `gap` event announces batches dropped while the
 * subscriber lagged, rendered as a marker instead of vanishing silently.
 */
function LiveTail() {
  const [entries, setEntries] = useState<TailEntry[]>([])
  const [connection, setConnection] = useState<Connection>('connecting')
  const [paused, setPaused] = useState(false)
  const pausedRef = useRef(paused)
  const nextIdRef = useRef(1)

  useEffect(() => {
    pausedRef.current = paused
  }, [paused])

  useEffect(() => {
    const source = new EventSource('/api/tail')

    source.onopen = () => setConnection('live')
    source.onerror = () => setConnection('closed')

    source.addEventListener('logs', (event) => {
      if (pausedRef.current) return

      let batch: QueryRow[] = []
      try {
        batch = JSON.parse((event as MessageEvent).data) as QueryRow[]
      } catch {
        return
      }

      setEntries((previous) => {
        // Batches arrive oldest-first within the batch; newest ends up on top.
        const incoming: TailEntry[] = batch.map((row) => ({
          kind: 'log',
          id: nextIdRef.current++,
          timestampUs: row.timestamp_us,
          level: String(row.level ?? '').toUpperCase(),
          service: String(row.service ?? '-'),
          message: String(row.message ?? ''),
        }))
        return [...incoming.reverse(), ...previous].slice(0, MAX_TAIL_ENTRIES)
      })
    })

    source.addEventListener('gap', (event) => {
      const skipped = Number((event as MessageEvent).data) || 0
      if (pausedRef.current || skipped <= 0) return

      setEntries((previous) =>
        [{ kind: 'gap' as const, id: nextIdRef.current++, skipped }, ...previous].slice(
          0,
          MAX_TAIL_ENTRIES,
        ),
      )
    })

    return () => source.close()
  }, [])

  const status = CONNECTION_STYLES[connection]

  return (
    <main className="flex min-h-0 flex-1 flex-col">
      <div className="flex shrink-0 items-center justify-between border-b border-zinc-800 bg-zinc-950 px-3 py-1.5">
        <div className="flex items-center gap-2 text-sm text-zinc-300">
          <span className={`h-2 w-2 rounded-full ${status.dot}`} />
          <span className="text-xs uppercase tracking-wide text-zinc-500">{status.label}</span>
          <span className="ml-2 text-xs text-zinc-500">{entries.length} buffered</span>
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => setPaused((value) => !value)}
            className="flex cursor-pointer items-center gap-1.5 rounded-md px-2 py-1 text-xs text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-200"
          >
            {paused ? <LuPlay className="h-3.5 w-3.5" /> : <LuPause className="h-3.5 w-3.5" />}
            {paused ? 'Resume' : 'Pause'}
          </button>
          <button
            type="button"
            onClick={() => setEntries([])}
            className="flex cursor-pointer items-center gap-1.5 rounded-md px-2 py-1 text-xs text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-200"
          >
            <LuTrash2 className="h-3.5 w-3.5" />
            Clear
          </button>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {entries.length === 0 ? (
          <div className="flex h-full items-center justify-center text-sm text-zinc-500">
            Waiting for logs…
          </div>
        ) : (
          entries.map((entry) =>
            entry.kind === 'gap' ? (
              <div key={entry.id} className="border-b border-amber-900/50 bg-amber-950/20 px-3 py-1 text-xs text-amber-400">
                … {entry.skipped} batch{entry.skipped === 1 ? '' : 'es'} dropped while the tail lagged …
              </div>
            ) : (
              <div key={entry.id} className="flex items-center gap-3 border-b border-zinc-800 px-3 py-1.5 text-sm">
                <span className="w-20 shrink-0 font-mono text-xs text-zinc-500">
                  {formatTimestamp(entry.timestampUs)}
                </span>
                <span className={`w-10 shrink-0 font-mono text-xs font-medium ${LEVEL_STYLES[entry.level] ?? 'text-zinc-400'}`}>
                  {entry.level}
                </span>
                <span className="mr-2 shrink-0 rounded-md border border-zinc-700 bg-zinc-800 px-2 py-0.5 text-xs text-zinc-300">
                  {entry.service}
                </span>
                <span className="min-w-0 flex-1 truncate font-mono text-zinc-300">{entry.message}</span>
              </div>
            ),
          )
        )}
      </div>
    </main>
  )
}

export default LiveTail
