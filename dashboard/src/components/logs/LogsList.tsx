import { useEffect, useMemo, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { FaInfoCircle } from 'react-icons/fa'
import { BsWindowFullscreen } from 'react-icons/bs'
import { LuChevronLeft, LuChevronRight, LuInfo, LuRefreshCw } from 'react-icons/lu'
import { RiMore2Fill } from 'react-icons/ri'
import Tooltip from '../Tooltip'
import EmptyState from '../EmptyState'
import Spinner from '../Spinner'
import { RANGE_SECONDS } from './Timeline'
import type { TimeRange } from '../Header'
import type { QueryRow } from '../../api/logs'
import { normalizeLevel, severityStyle } from '../../utils/severity'

const RANGE_TO_LABEL: Record<TimeRange, string> = {
  '15m': '15 minutes',
  '1h': '1 hour',
  '3h': '3 hours',
  '6h': '6 hours',
  '12h': '12 hours',
  '24h': '24 hours',
  '7d': '7 days',
  '30d': '30 days',
}

interface LogsListProps {
  onToggleFullscreen: () => void
  range: TimeRange
  shift: number
  logs?: QueryRow[]
  /** True while the query is in flight and no rows are on screen yet. */
  isLoading?: boolean
  /** Zero-based page index; page N starts at row N * pageSize. */
  page: number
  pageSize: number
  onPageChange: (page: number) => void
  onRefresh: () => void
}

interface LogPayload {
  [key: string]: any
}

interface JsonViewerProps {
  data: LogPayload
  initiallyExpanded?: boolean
}

export function LogDetailsViewer({
  data,
  initiallyExpanded = true,
}: JsonViewerProps) {
  return (
    <div className="border-b border-[#262626] bg-[#111111] p-4 font-mono text-[13px] leading-6">
      <JsonNode
        label="payload"
        value={data}
        defaultOpen={initiallyExpanded}
        isRoot={true}
      />
    </div>
  )
}

function JsonNode({ label, value, defaultOpen, isRoot = false }: any) {
  const [isOpen, setIsOpen] = useState(defaultOpen)

  const isObject = value !== null && typeof value === 'object'
  const isArray = Array.isArray(value)

  if (!isObject) {
    return (
      <div className="ml-4 flex">
        <span className="min-w-[120px] text-[#3b82f6]">{label}:</span>
        <span
          className={typeof value === 'number' ? 'text-[#f59e0b]' : 'text-[#eab308]'}
        >
          {typeof value === 'string' ? `"${value}"` : String(value)}
        </span>
      </div>
    )
  }

  const keys = Object.keys(value)
  const bracketOpen = isArray ? '[' : '{'
  const bracketClose = isArray ? ']' : '}'

  return (
    <div className={isRoot ? '' : 'ml-4'}>
      <div
        className="flex cursor-pointer select-none items-center text-[#888888] hover:text-white"
        onClick={() => setIsOpen(!isOpen)}
      >
        <span className="mr-2 inline-block w-3 text-center">
          {isOpen ? '▾' : '▸'}
        </span>
        {label !== 'payload' && (
          <span className="mr-2 text-[#3b82f6]">{label}:</span>
        )}
        <span>
          {bracketOpen} {isOpen ? '' : `... ${keys.length} items ${bracketClose}`}
        </span>
      </div>

      {isOpen && (
        <div>
          {keys.map((key: string) => (
            <JsonNode
              key={key}
              label={key}
              value={value[key as keyof typeof value]}
              defaultOpen={defaultOpen}
            />
          ))}
          <div className="ml-5 text-[#888888]">{bracketClose}</div>
        </div>
      )}
    </div>
  )
}

interface LogRow {
  id: number
  timestamp: string
  level: string
  service: string
  message: string
  details: Record<string, unknown>
}

const MONTHS = [
  'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun',
  'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec',
]

function formatFooterDate(timestamp: number): string {
  const date = new Date(timestamp * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${MONTHS[date.getMonth()]} ${date.getDate()}, ${date.getFullYear()} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}

function resolveEpochMillis(timestampUs: unknown): number {
  if (typeof timestampUs === 'number') {
    return Math.floor(timestampUs / 1000)
  }
  if (typeof timestampUs === 'string') {
    // arrow-json serializes timestamp columns as naive-UTC ISO-8601 strings
    // (e.g. "2026-08-14T10:51:45.717136"); parse them as UTC epochs.
    const iso = /Z$|[+-]\d{2}:\d{2}$/.test(timestampUs)
      ? timestampUs
      : `${timestampUs}Z`
    const ms = Date.parse(iso)
    if (!Number.isNaN(ms)) return ms
  }
  return NaN
}

function formatRowTimestamp(timestampUs: unknown): string {
  const ms = resolveEpochMillis(timestampUs)
  if (!Number.isFinite(ms)) return ''
  const date = new Date(ms)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${String(date.getMilliseconds()).padStart(3, '0')}`
}

/** Parses a record's raw JSON body into an object tree for the details view. */
export function parseRawBody(rawBody: unknown): Record<string, unknown> {
  if (typeof rawBody !== 'string') return {}
  try {
    const parsed: unknown = JSON.parse(rawBody)
    return parsed && typeof parsed === 'object'
      ? (parsed as Record<string, unknown>)
      : {}
  } catch {
    return {}
  }
}

const CSV_COLUMNS = ['timestamp_us', 'trace_id', 'level', 'service', 'message', 'raw_body']

/** Quotes a CSV cell when it contains separators, quotes or newlines. */
function escapeCsvCell(value: unknown): string {
  const cell = String(value ?? '')
  if (cell.includes('"') || cell.includes(',') || cell.includes('\n')) {
    return `"${cell.replace(/"/g, '""')}"`
  }
  return cell
}

function downloadLogs(logs: QueryRow[], extension: 'csv' | 'json', mimeType: string) {
  const content =
    extension === 'csv'
      ? [CSV_COLUMNS.join(','), ...logs.map((row) => CSV_COLUMNS.map((column) => escapeCsvCell(row[column])).join(','))].join('\n')
      : JSON.stringify(logs, null, 2)
  const blob = new Blob([content], { type: mimeType })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = `greplog-logs-${Date.now()}.${extension}`
  anchor.click()
  URL.revokeObjectURL(url)
}

function LogsList({
  onToggleFullscreen,
  range,
  shift,
  logs,
  isLoading = false,
  page,
  pageSize,
  onPageChange,
  onRefresh,
}: LogsListProps) {
  const [actionsOpen, setActionsOpen] = useState(false)
  const [expandedIds, setExpandedIds] = useState<Set<number>>(new Set())
  const parentRef = useRef<HTMLDivElement>(null)
  const rows: LogRow[] = useMemo(
    () =>
      (logs ?? []).map((row, index) => ({
        id: index + 1,
        timestamp: formatRowTimestamp(row.timestamp_us),
        level: normalizeLevel(row.level),
        service: String(row.service ?? '-'),
        message: String(row.message ?? ''),
        details: parseRawBody(row.raw_body),
      })),
    [logs],
  )
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 40,
    overscan: 8,
  })

  useEffect(() => {
    setExpandedIds(new Set())
    parentRef.current?.scrollTo({ top: 0 })
  }, [logs])

  const now = Math.floor(Date.now() / 1000)
  const rangeEnd = now - shift
  const rangeStart = rangeEnd - RANGE_SECONDS[range]

  const toggleExpand = (id: number) => {
    setExpandedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const hasPreviousPage = page > 0
  // A short page means the window is exhausted; only a full page can have a successor.
  const mightHaveNextPage = (logs?.length ?? 0) >= pageSize
  const goToPreviousPage = () => {
    if (hasPreviousPage) onPageChange(page - 1)
  }
  const goToNextPage = () => {
    if (mightHaveNextPage) onPageChange(page + 1)
  }
  const refreshFromMenu = () => {
    onRefresh()
    setActionsOpen(false)
  }
  const exportCsvFromMenu = () => {
    if (logs && logs.length > 0) downloadLogs(logs, 'csv', 'text/csv;charset=utf-8')
    setActionsOpen(false)
  }
  const exportJsonFromMenu = () => {
    if (logs && logs.length > 0) downloadLogs(logs, 'json', 'application/json;charset=utf-8')
    setActionsOpen(false)
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 items-center justify-between border-b border-zinc-800 bg-zinc-950 px-3 py-1 z-10">
        <p className="text-sm text-zinc-300">
          <span className="font-medium text-zinc-100">{rows.length}</span>{' '}
          Logs
        </p>
        <div className="flex items-center gap-2">
          <div className="h-4 w-px bg-zinc-700" />
          <Tooltip content="Toggle fullscreen" side="bottom">
            <button
              type="button"
              onClick={onToggleFullscreen}
              className="cursor-pointer rounded-md p-1.5 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300"
            >
              <BsWindowFullscreen className="h-4 w-4" />
            </button>
          </Tooltip>
          <div className="h-4 w-px bg-zinc-700" />
          <div className="relative">
            <Tooltip content="Actions" side="bottom-left">
              <button
                type="button"
                onClick={() => setActionsOpen((value) => !value)}
                className="cursor-pointer rounded-md p-1.5 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300"
              >
                <RiMore2Fill className="h-4 w-4" />
              </button>
            </Tooltip>
            {actionsOpen && (
              <>
                <div
                  className="fixed inset-0 z-10"
                  onClick={() => setActionsOpen(false)}
                />
                <ul className="absolute right-0 top-full z-20 mt-1 w-44 rounded-md border border-zinc-700 bg-zinc-900 py-1 text-sm shadow-lg">
                  <li>
                    <button
                      type="button"
                      disabled={!logs || logs.length === 0}
                      onClick={exportCsvFromMenu}
                      className="flex w-full cursor-pointer items-center px-3 py-1.5 text-left text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100 disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      Export CSV
                    </button>
                  </li>
                  <li>
                    <button
                      type="button"
                      disabled={!logs || logs.length === 0}
                      onClick={exportJsonFromMenu}
                      className="flex w-full cursor-pointer items-center px-3 py-1.5 text-left text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100 disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      Export JSON
                    </button>
                  </li>
                  <li>
                    <button
                      type="button"
                      onClick={refreshFromMenu}
                      className="flex w-full cursor-pointer items-center gap-2 px-3 py-1.5 text-left text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
                    >
                      <LuRefreshCw className="h-3.5 w-3.5" />
                      Refresh
                    </button>
                  </li>
                </ul>
              </>
            )}
          </div>
        </div>
      </div>
      <div ref={parentRef} className="min-h-0 flex-1 overflow-y-auto">
        {rows.length === 0 ? (
          <div className="flex h-full items-center justify-center">
            {isLoading ? (
              <Spinner className="h-6 w-6" />
            ) : (
              <EmptyState
                title="No logs found"
                description={
                  <>
                    There are no logs matching your current filters in the last{' '}
                    <span className="text-zinc-300">{RANGE_TO_LABEL[range]}</span>. Try
                    clearing your search or expanding the time range.
                  </>
                }
              />
            )}
          </div>
        ) : (
        <div
          className="relative w-full"
          style={{ height: virtualizer.getTotalSize() }}
        >
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const log = rows[virtualRow.index]
            const isExpanded = expandedIds.has(log.id)
            return (
              <div
                key={log.id}
                data-index={virtualRow.index}
                ref={virtualizer.measureElement}
                className="absolute left-0 top-0 w-full border-b border-zinc-800 text-sm"
                style={{ transform: `translateY(${virtualRow.start}px)` }}
              >
                <div className="flex items-center gap-3 px-3 py-1.5">
                  <button
                    type="button"
                    onClick={() => toggleExpand(log.id)}
                    className="cursor-pointer rounded p-0.5 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300"
                  >
                    <LuChevronRight
                      className={`h-4 w-4 transition-transform ${
                        isExpanded ? 'rotate-90' : ''
                      }`}
                    />
                  </button>
                  <span className="w-20 shrink-0 font-mono text-xs text-zinc-500">
                    {log.timestamp}
                  </span>
                  <span className="w-6 shrink-0" />
                  <span className="flex w-8 shrink-0 items-center">
                    <FaInfoCircle
                      className={`h-3.5 w-3.5 ${severityStyle(log.level).text}`}
                    />
                  </span>
                  <span className="mr-2 shrink-0 rounded-md border border-zinc-700 bg-zinc-800 px-2 py-0.5 text-xs text-zinc-300">
                    {log.service}
                  </span>
                  <span className="min-w-0 flex-1 truncate font-mono text-zinc-300">
                    {log.message}
                  </span>
                </div>
                {isExpanded && <LogDetailsViewer data={log.details} />}
              </div>
            )
          })}
        </div>
        )}
      </div>
      <footer className="flex shrink-0 items-center justify-between gap-2 border-t border-zinc-800 bg-zinc-950 px-3 py-1.5 text-xs text-zinc-400 z-10">
        <div className="flex min-w-0 items-center gap-2">
          <LuInfo className="h-3.5 w-3.5 shrink-0 text-zinc-500" />
          <span className="min-w-0 truncate">
            Showing logs from{' '}
            <span className="font-medium text-zinc-200">
              {formatFooterDate(rangeStart)}
            </span>{' '}
            to{' '}
            <span className="font-medium text-zinc-200">
              {formatFooterDate(rangeEnd)}
            </span>
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <span>
            Page <span className="font-medium text-zinc-300">{page + 1}</span>
          </span>
          <button
            type="button"
            onClick={goToPreviousPage}
            disabled={!hasPreviousPage}
            aria-label="Previous page"
            className="inline-flex h-7 cursor-pointer items-center justify-center rounded-md border border-zinc-700 bg-zinc-900 px-2 text-zinc-300 transition-colors hover:bg-zinc-800 hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
          >
            <LuChevronLeft className="h-3.5 w-3.5" />
            Prev
          </button>
          <button
            type="button"
            onClick={goToNextPage}
            disabled={!mightHaveNextPage}
            aria-label="Next page"
            className="inline-flex h-7 cursor-pointer items-center justify-center rounded-md border border-zinc-700 bg-zinc-900 px-2 text-zinc-300 transition-colors hover:bg-zinc-800 hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
          >
            Next
            <LuChevronRight className="h-3.5 w-3.5" />
          </button>
        </div>
      </footer>
    </div>
  )
}

export default LogsList
