import { useEffect, useMemo, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { FaInfoCircle } from 'react-icons/fa'
import { BsWindowFullscreen } from 'react-icons/bs'
import { LuChevronRight, LuInfo } from 'react-icons/lu'
import { RiMore2Fill } from 'react-icons/ri'
import Tooltip from '../Tooltip'
import EmptyIcon from '../icons/EmptyIcon'
import { RANGE_SECONDS } from './Timeline'
import type { TimeRange } from '../Header'
import type { QueryRow } from '../../api/logs'

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

type LogLevel = 'DEBUG' | 'INFO' | 'WARN' | 'ERROR'

interface LogRow {
  id: number
  timestamp: string
  level: LogLevel
  service: string
  message: string
  details: Record<string, unknown>
}

const LEVEL_STYLES: Record<LogLevel, string> = {
  DEBUG: 'text-zinc-500',
  INFO: 'text-sky-400',
  WARN: 'text-amber-400',
  ERROR: 'text-red-400',
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

function formatRowTimestamp(timestampUs: unknown): string {
  const us = Number(timestampUs)
  if (!Number.isFinite(us)) return ''
  const date = new Date(Math.floor(us / 1000))
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${String(date.getMilliseconds()).padStart(3, '0')}`
}

function parseRawBody(rawBody: unknown): Record<string, unknown> {
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

const LOG_LEVELS: LogLevel[] = ['DEBUG', 'INFO', 'WARN', 'ERROR']

function LogsList({
  onToggleFullscreen,
  range,
  shift,
  logs,
}: LogsListProps) {
  const [actionsOpen, setActionsOpen] = useState(false)
  const [expandedIds, setExpandedIds] = useState<Set<number>>(new Set())
  const parentRef = useRef<HTMLDivElement>(null)
  const rows: LogRow[] = useMemo(
    () =>
      (logs ?? []).map((row, index) => ({
        id: index + 1,
        timestamp: formatRowTimestamp(row.timestamp_us),
        level: LOG_LEVELS.includes(row.level as LogLevel)
          ? (row.level as LogLevel)
          : 'INFO',
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
                <ul className="absolute right-0 top-full z-20 mt-1 w-40 rounded-md border border-zinc-700 bg-zinc-900 py-1 text-sm shadow-lg">
                  {['Export CSV', 'Export JSON', 'Refresh'].map((action) => (
                    <li key={action}>
                      <button
                        type="button"
                        onClick={() => setActionsOpen(false)}
                        className="flex w-full cursor-pointer items-center px-3 py-1.5 text-left text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
                      >
                        {action}
                      </button>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </div>
        </div>
      </div>
      <div ref={parentRef} className="min-h-0 flex-1 overflow-y-auto">
        {rows.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
            <EmptyIcon className="h-14 w-14 text-zinc-600" />
            <div>
              <p className="text-base font-medium text-zinc-100">No logs found</p>
              <p className="mt-1 text-sm font-medium text-zinc-400">
                There are no logs matching your current filters in the last{' '}
                <span className="text-zinc-300">{RANGE_TO_LABEL[range]}</span>.
              </p>
              <p className="mt-1 text-sm font-medium text-zinc-400">
                Try clearing your search or expanding the time range.{' '}
              </p>
            </div>
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
                      className={`h-3.5 w-3.5 ${LEVEL_STYLES[log.level]}`}
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
      <footer className="flex shrink-0 items-center gap-2 border-t border-zinc-800 px-3 py-1.5 text-xs text-zinc-400 z-10 bg-zinc-950">
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
      </footer>
    </div>
  )
}

export default LogsList
