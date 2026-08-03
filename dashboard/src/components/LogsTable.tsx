import { useState, useMemo } from 'react'
import { LuCircleCheck, LuDownload, LuChevronLeft, LuChevronRight, LuArrowUpDown } from 'react-icons/lu'
import Dropdown from './Dropdown.tsx'
import TopLoadingBar from './TopLoadingBar.tsx'
import type { LogEntry } from '../types/index.ts'

interface LogsTableProps {
  data: LogEntry[]
  totalRows?: number
  totalLogs?: number
  querySeconds?: number
  filteredServices?: string[]
  onView?: (row: LogEntry) => void
  isFetching?: boolean
}

function getLevelColors(level: string) {
  const normalized = level.toLowerCase()
  if (normalized === 'error' || normalized === 'critical' || normalized === 'fatal') {
    return {
      border: '#ef4444',
      bg: 'rgba(239, 68, 68, 0.12)',
      text: '#ef4444',
    }
  }
  if (normalized === 'warn' || normalized === 'warning') {
    return {
      border: '#f59e0b',
      bg: 'rgba(245, 158, 11, 0.12)',
      text: '#fbbf24',
    }
  }
  if (normalized === 'info') {
    return {
      border: '#10b981',
      bg: 'rgba(16, 185, 129, 0.12)',
      text: '#34d399',
    }
  }
  if (normalized === 'debug') {
    return {
      border: '#3b82f6',
      bg: 'rgba(59, 130, 246, 0.12)',
      text: '#60a5fa',
    }
  }
  return {
    border: '#9ca3af',
    bg: 'rgba(156, 163, 175, 0.12)',
    text: '#9ca3af',
  }
}

function getStatusColor(code: number): string {
  if (code >= 500) return '#ef4444'
  if (code >= 400) return '#f59e0b'
  if (code >= 300) return '#3b82f6'
  return '#10b981'
}

export default function LogsTable({ data, totalRows: totalRowsProp, totalLogs: totalLogsProp, querySeconds: secondsProp, filteredServices, onView, isFetching }: LogsTableProps) {
  const totalRows = totalRowsProp ?? data.length
  const totalLogs = totalLogsProp ?? data.length
  const seconds = secondsProp ?? 0.3
  const limits = ['500', '1k', '5k', '10k']
  const [limit, setLimit] = useState('500')
  const [sortDirection, setSortDirection] = useState<'asc' | 'desc'>('desc')
  const [page, setPage] = useState(0)

  const filtered = filteredServices
    ? data.filter((row) => filteredServices.includes(row.service))
    : data

  const sortedData = useMemo(() => {
    return [...filtered].sort((a, b) => {
      const aVal = a.timestamp
      const bVal = b.timestamp
      if (aVal < bVal) return sortDirection === 'asc' ? -1 : 1
      if (aVal > bVal) return sortDirection === 'asc' ? 1 : -1
      return 0
    })
  }, [filtered, sortDirection])

  const parsedLimit = limit === 'All' ? sortedData.length : parseInt(limit.replace('k', '000'), 10)
  const displayData = useMemo(() => sortedData.slice(0, parsedLimit), [sortedData, parsedLimit])

  const pageSize = limit === 'All' ? Math.max(displayData.length, 1) : parseInt(limit.replace('k', '000'), 10)
  const totalPages = Math.ceil(displayData.length / pageSize)
  const currentPage = Math.min(page, Math.max(totalPages - 1, 0))
  const pageData = useMemo(() => displayData.slice(currentPage * pageSize, (currentPage + 1) * pageSize), [displayData, currentPage, pageSize])

  const bodyRows = useMemo(() => pageData.map((row) => {
    const levelColors = getLevelColors(row.level)
    const statusColor = getStatusColor(row.statusCode)

    // Helper to safely format strings inside JSON output
    const escapeStr = (str: string) => str.replace(/\\/g, '\\\\').replace(/"/g, '\\"')

return (
          <div
            key={row.id}
            className="group relative flex items-start gap-4 py-2.5 pl-4 pr-3 border-b text-xs hover:bg-[var(--hover-bg-subtle)] transition-colors font-mono font-medium cursor-pointer"
            style={{ borderColor: 'var(--border-primary)' }}
            onClick={() => onView?.(row)}
          >
        {/* Level Stripe Indicator */}
        <div
          className="absolute left-0 top-0 bottom-0 w-[4px]"
          style={{ backgroundColor: levelColors.border }}
        />

        {/* View Action / Hover Indicator */}
        <div className="flex items-center gap-1.5 shrink-0 select-none">
          <span className="text-[var(--accent)] hover:underline font-bold cursor-pointer">
            View
          </span>
        </div>

        {/* Monospace JSON-like Structured Block */}
        <div className="flex-1 min-w-0 select-all font-mono font-medium leading-relaxed text-text-primary whitespace-pre-wrap break-all">
          <span className="text-text-secondary/70">{'{'}</span>{' '}
          
          <span className="text-[#a5d6ff]">"time"</span>
          <span className="text-text-secondary/80">:</span>{' '}
          <span className="text-[#79c0ff]">"{escapeStr(row.timestamp)}"</span>
          <span className="text-text-secondary/70">,</span>{' '}
          
          <span className="text-[#a5d6ff]">"level"</span>
          <span className="text-text-secondary/80">:</span>{' '}
          <span className="font-bold" style={{ color: levelColors.text }}>"{escapeStr(row.level.toUpperCase())}"</span>
          <span className="text-text-secondary/70">,</span>{' '}
          
          {row.service && (
            <>
              <span className="text-[#a5d6ff]">"service"</span>
              <span className="text-text-secondary/80">:</span>{' '}
              <span className="text-[#ffa657]">"{escapeStr(row.service)}"</span>
              <span className="text-text-secondary/70">,</span>{' '}
            </>
          )}

          {row.statusCode > 0 && (
            <>
              <span className="text-[#a5d6ff]">"status"</span>
              <span className="text-text-secondary/80">:</span>{' '}
              <span className="font-bold" style={{ color: statusColor }}>{row.statusCode}</span>
              <span className="text-text-secondary/70">,</span>{' '}
            </>
          )}

          {row.logger && (
            <>
              <span className="text-[#a5d6ff]">"logger"</span>
              <span className="text-text-secondary/80">:</span>{' '}
              <span className="text-[#ffa657]">"{escapeStr(row.logger)}"</span>
              <span className="text-text-secondary/70">,</span>{' '}
            </>
          )}

          {row.correlationId && (
            <>
              <span className="text-[#a5d6ff]">"trace_id"</span>
              <span className="text-text-secondary/80">:</span>{' '}
              <span className="text-[#ffa657]">"{escapeStr(row.correlationId)}"</span>
              <span className="text-text-secondary/70">,</span>{' '}
            </>
          )}

          <span className="text-[#a5d6ff]">"message"</span>
          <span className="text-text-secondary/80">:</span>{' '}
          <span className="text-[#7ee787]">"{escapeStr(row.message)}"</span>
          
          <span className="text-text-secondary/70">{' }'}</span>
        </div>
      </div>
    )
  }), [pageData, onView])

  function exportToCSV() {
    const columnsToExport = ['timestamp', 'level', 'service', 'statusCode', 'message']
    const headers = ['timestamp', 'level', 'service_name', 'status_code', 'message']
    const rows = displayData.map(row =>
      columnsToExport.map(key => {
        const val = row[key as keyof typeof row] ?? ''
        return `"${String(val).replace(/"/g, '""')}"`
      }).join(',')
    )
    const csv = [headers.join(','), ...rows].join('\n')
    const blob = new Blob([csv], { type: 'text/csv' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.download = 'logs-export.csv'
    a.href = url
    a.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="flex-1 min-h-0 min-w-0 overflow-hidden px-2 pt-1.5 pb-2">
      <div className="h-full w-full border flex flex-col min-w-0 overflow-hidden relative" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
        <TopLoadingBar active={!!isFetching} />
        {/* Toolbar */}
        <div className="flex items-center border-b shrink-0 z-10" style={{ borderColor: 'var(--border-primary)', backgroundColor: 'var(--bg-secondary)' }}>
          <div className="flex items-center gap-2 px-3 py-1.5 flex-1">
            <LuCircleCheck className="size-4 text-success" />
            <span className="text-sm font-medium text-text-secondary font-mono">
              {totalRows.toLocaleString()} of {totalLogs.toLocaleString()} Rows in {seconds}s
            </span>
          </div>

          <div className="h-4 w-px shrink-0" style={{ backgroundColor: 'var(--border-primary)' }} />

          {/* Sort Button */}
          <div className="px-3 py-1.5 shrink-0">
            <button
              onClick={() => setSortDirection((prev) => (prev === 'desc' ? 'asc' : 'desc'))}
              className="flex items-center gap-1.5 px-2.5 py-1 text-sm text-text-secondary rounded hover:bg-[var(--hover-bg)] transition-colors border cursor-pointer"
              style={{ borderColor: 'var(--border-primary)' }}
            >
              <LuArrowUpDown className="size-3.5" />
              <span>Time:</span>
              <span className="font-semibold text-text-primary">
                {sortDirection === 'desc' ? 'Newest' : 'Oldest'}
              </span>
            </button>
          </div>

          <div className="h-4 w-px shrink-0" style={{ backgroundColor: 'var(--border-primary)' }} />

          {/* Limit selector */}
          <div className="flex items-center gap-1.5 px-3 py-1.5 w-[130px] shrink-0">
            <span className="text-sm text-text-secondary">Limit:</span>
            <Dropdown
              trigger={<span>{limit}</span>}
              items={limits.map((opt) => ({ label: opt, value: opt }))}
              value={limit}
              onChange={(value) => {
                setLimit(value)
                setPage(0)
              }}
              minWidth="min-w-20"
            />
          </div>

          <div className="h-4 w-px shrink-0" style={{ backgroundColor: 'var(--border-primary)' }} />

          {/* Export Button */}
          <div className="px-3 py-1.5 shrink-0">
            <button onClick={exportToCSV} className="flex items-center gap-1.5 px-2 py-1 text-sm text-text-secondary rounded hover:bg-[var(--hover-bg)] transition-colors cursor-pointer">
              <LuDownload className="size-3.5" />
              Export
            </button>
          </div>

          <div className="h-4 w-px shrink-0" style={{ backgroundColor: 'var(--border-primary)' }} />

          {/* Pagination */}
          <div className="px-3 py-1.5 shrink-0 flex items-center gap-1.5">
            <button
              className="flex items-center justify-center size-7 rounded hover:bg-[var(--hover-bg)] transition-colors disabled:opacity-30"
              disabled={currentPage === 0}
              onClick={() => setPage((p) => Math.max(0, p - 1))}
            >
              <LuChevronLeft className="size-3.5" style={{ color: 'var(--text-secondary)' }} />
            </button>
            <span className="text-xs text-text-secondary whitespace-nowrap">
              {totalPages > 0 ? `${currentPage + 1} / ${totalPages}` : '-'}
            </span>
            <button
              className="flex items-center justify-center size-7 rounded hover:bg-[var(--hover-bg)] transition-colors disabled:opacity-30"
              disabled={currentPage >= totalPages - 1}
              onClick={() => setPage((p) => Math.min(Math.max(totalPages - 1, 0), p + 1))}
            >
              <LuChevronRight className="size-3.5" style={{ color: 'var(--text-secondary)' }} />
            </button>
          </div>
        </div>

        {/* Log List View */}
        <div className="flex-1 overflow-auto min-h-0 relative">
          <div className="flex flex-col min-w-0">
            {bodyRows.length > 0 ? (
              bodyRows
            ) : (
              <div className="text-center text-text-secondary py-8 text-sm">
                No logs matching current filters found.
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
