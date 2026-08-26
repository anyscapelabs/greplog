import { useEffect, useRef, useState } from 'react'
import { LuChevronRight } from 'react-icons/lu'
import { RiDeleteBinLine, RiPauseLine, RiPlayLine } from 'react-icons/ri'
import { LogDetailsViewer, parseRawBody } from '../components/logs/LogsList'
import type { QueryRow } from '../api/logs'
import { severityStyle } from '../utils/severity'

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
  details: Record<string, unknown>
}

interface GapEntry {
  kind: 'gap'
  id: number
  skipped: number
}

type TailEntry = LogEntry | GapEntry

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
  const [expandedIds, setExpandedIds] = useState<Set<number>>(new Set())
  const pausedRef = useRef(paused)
  const nextIdRef = useRef(1)

  const toggleExpand = (id: number) => {
    setExpandedIds((previous) => {
      const next = new Set(previous)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const clearEntries = () => {
    setEntries([])
    setExpandedIds(new Set())
  }

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
          details: parseRawBody(row.raw_body),
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
        <div className="flex items-center gap-3">
          <p className="text-sm text-zinc-300">
            <span className="font-medium text-zinc-100">{entries.length}</span> Logs
          </p>
          <div className="h-4 w-px bg-zinc-700" />
          <div className="flex items-center gap-2">
            <span className={`h-2 w-2 rounded-full ${status.dot}`} />
            <span className="text-xs uppercase tracking-wide text-zinc-500">{status.label}</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setPaused((value) => !value)}
            className="flex cursor-pointer items-center gap-1.5 rounded-md border border-zinc-700 px-3 py-1 text-sm font-medium text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
          >
            {paused ? <RiPlayLine className="h-4 w-4" /> : <RiPauseLine className="h-4 w-4" />}
            {paused ? 'Resume' : 'Pause'}
          </button>
          <button
            type="button"
            onClick={clearEntries}
            className="flex cursor-pointer items-center gap-1.5 rounded-md border border-zinc-700 px-3 py-1 text-sm font-medium text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
          >
            <RiDeleteBinLine className="h-4 w-4" />
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
          entries.map((entry) => {
            if (entry.kind === 'gap') {
              return (
                <div key={entry.id} className="border-b border-amber-900/50 bg-amber-950/20 px-3 py-1 text-xs text-amber-400">
                  … {entry.skipped} batch{entry.skipped === 1 ? '' : 'es'} dropped while the tail lagged …
                </div>
              )
            }

            const isExpanded = expandedIds.has(entry.id)
            return (
              <div key={entry.id} className="border-b border-zinc-800">
                <div className="flex items-center gap-3 px-3 py-1.5 text-sm">
                  <button
                    type="button"
                    onClick={() => toggleExpand(entry.id)}
                    className="cursor-pointer rounded p-0.5 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300"
                  >
                    <LuChevronRight
                      className={`h-4 w-4 transition-transform ${
                        isExpanded ? 'rotate-90' : ''
                      }`}
                    />
                  </button>
                  <span className="w-20 shrink-0 font-mono text-xs text-zinc-500">
                    {formatTimestamp(entry.timestampUs)}
                  </span>
                  <span className={`w-16 shrink-0 truncate font-mono text-xs font-medium ${severityStyle(entry.level).text}`}>
                    {entry.level}
                  </span>
                  <span className="mr-2 shrink-0 rounded-md border border-zinc-700 bg-zinc-800 px-2 py-0.5 text-xs text-zinc-300">
                    {entry.service}
                  </span>
                  <span className="min-w-0 flex-1 truncate font-mono text-zinc-300">{entry.message}</span>
                </div>
                {isExpanded && <LogDetailsViewer data={entry.details} />}
              </div>
            )
          })
        )}
      </div>
    </main>
  )
}

export default LiveTail
