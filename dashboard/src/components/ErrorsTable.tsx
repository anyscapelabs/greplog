import { useState, useMemo, useEffect } from 'react'
import { LuChevronDown, LuChevronUp, LuChevronLeft, LuChevronRight, LuCircleCheck, LuDownload } from 'react-icons/lu'
import Dropdown from './Dropdown.tsx'
import type { ErrorEntry } from '../types/index.ts'

const columns = [
  { key: 'timestamp', label: 'timestamp', width: 'w-[180px]' },
  { key: 'level', label: 'level', width: 'w-[90px]' },
  { key: 'service', label: 'service_name', width: 'w-[130px]' },
  { key: 'errorCode', label: 'error_code', width: 'w-[160px]' },
  { key: 'message', label: 'message', width: 'w-[400px]' },
  { key: 'latency', label: 'latency', width: 'w-[100px]' },
  { key: 'freq', label: 'freq', width: 'w-[90px]' },
]

interface ErrorsTableProps {
  data: ErrorEntry[]
  totalRows?: number
  totalLogs?: number
  querySeconds?: number
  filteredServices?: string[]
  onView?: (row: ErrorEntry) => void
}

export default function ErrorsTable({ data, totalRows: totalRowsProp, totalLogs: totalLogsProp, querySeconds: secondsProp, filteredServices, onView }: ErrorsTableProps) {
  const [totalRows] = useState(totalRowsProp ?? data.length)
  const [totalLogs] = useState(totalLogsProp ?? data.length)
  const [seconds] = useState(secondsProp ?? 0.3)
  const limits = ['500', '1k', '5k', '10k']
  const [limit, setLimit] = useState('500')
  const [sortColumn, setSortColumn] = useState<string | null>(null)
  const [sortDirection, setSortDirection] = useState<'asc' | 'desc'>('asc')
  const [page, setPage] = useState(0)

  const filtered = filteredServices
    ? data.filter((row) => filteredServices.includes(row.service))
    : data

  const sortedData = useMemo(() => {
    if (!sortColumn) return filtered
    return [...filtered].sort((a, b) => {
      const aVal = a[sortColumn as keyof typeof a]
      const bVal = b[sortColumn as keyof typeof b]
      if (typeof aVal === 'number' && typeof bVal === 'number') {
        return sortDirection === 'asc' ? aVal - bVal : bVal - aVal
      }
      const aStr = String(aVal)
      const bStr = String(bVal)
      if (aStr < bStr) return sortDirection === 'asc' ? -1 : 1
      if (aStr > bStr) return sortDirection === 'asc' ? 1 : -1
      return 0
    })
  }, [filtered, sortColumn, sortDirection])

  const parsedLimit = limit === 'All' ? sortedData.length : parseInt(limit.replace('k', '000'))
  const displayData = useMemo(() => sortedData.slice(0, parsedLimit), [sortedData, parsedLimit])

  const pageSize = limit === 'All' ? displayData.length : parseInt(limit.replace('k', '000'))
  const totalPages = Math.ceil(displayData.length / pageSize)
  const pageData = useMemo(() => displayData.slice(page * pageSize, (page + 1) * pageSize), [displayData, page, pageSize])

  const bodyRows = useMemo(() => pageData.map((row) => {
    const levelColor = row.level === 'critical' ? 'var(--error)' : row.level === 'error' ? 'var(--error)' : row.level === 'warn' ? 'var(--warn)' : 'var(--text-secondary)'
    return (
      <div
        key={row.id}
        className="flex items-center border-b text-sm hover:bg-[var(--hover-bg-subtle)] transition-colors font-mono h-9"
        style={{ borderColor: 'var(--border-primary)' }}
      >
        <div className="w-[60px] shrink-0 h-full border-r border-l flex items-center justify-center" style={{ borderColor: 'var(--border-primary)' }}>
          <button className="text-sm text-[var(--accent)] hover:text-[var(--accent)] transition-colors cursor-pointer" onClick={() => onView?.(row)}>View</button>
        </div>
        <div className="w-[180px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{row.timestamp}</div>
        <div className="w-[90px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center" style={{ color: levelColor, borderColor: 'var(--border-primary)' }}>{row.level}</div>
        <div className="w-[130px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{row.service}</div>
        <div className="w-[160px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center" style={{ color: 'var(--error)', borderColor: 'var(--border-primary)' }}>{row.errorCode}</div>
        <div className="w-[400px] shrink-0 px-3 text-text-primary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{row.message}</div>
        <div className="w-[100px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{row.latency}</div>
        <div className="w-[90px] shrink-0 px-3 text-text-primary truncate h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{row.freq}x</div>
      </div>
    )
  }), [pageData, onView])

  useEffect(() => { setPage(0) }, [limit, sortColumn, sortDirection, filteredServices])

  function handleSort(column: string) {
    if (sortColumn !== column) {
      setSortColumn(column)
      setSortDirection('asc')
    } else if (sortDirection === 'asc') {
      setSortDirection('desc')
    } else {
      setSortColumn(null)
      setSortDirection('asc')
    }
  }

  function exportToCSV() {
    const headers = columns.map(c => c.label)
    const rows = displayData.map(row =>
      columns.map(c => String(row[c.key as keyof typeof row] ?? '')).join(',')
    )
    const csv = [headers.join(','), ...rows].join('\n')
    const blob = new Blob([csv], { type: 'text/csv' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.download = 'errors-export.csv'
    a.href = url
    a.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="flex-1 min-h-0 min-w-0 overflow-hidden px-2 pt-1.5 pb-2">
      <div className="h-full w-full border flex flex-col min-w-0 overflow-hidden" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
        <div className="flex items-center border-b shrink-0 z-10 relative" style={{ borderColor: 'var(--border-primary)', backgroundColor: 'var(--bg-secondary)' }}>
          <div className="flex items-center gap-2 px-3 py-1.5 flex-1">
            <LuCircleCheck className="size-4 text-success" />
            <span className="text-sm font-medium text-text-secondary">
              {totalRows.toLocaleString()} of {totalLogs.toLocaleString()} Rows in {seconds}s
            </span>
          </div>
          <div className="h-4 w-px shrink-0" style={{ backgroundColor: 'var(--border-primary)' }} />
          <div className="flex items-center gap-1.5 px-3 py-1.5 w-[130px] shrink-0">
            <span className="text-sm text-text-secondary">Limit:</span>
            <Dropdown
              trigger={<span>{limit}</span>}
              items={limits.map((opt) => ({ label: opt, value: opt }))}
              value={limit}
              onChange={setLimit}
              minWidth="min-w-20"
            />
          </div>
          <div className="h-4 w-px shrink-0" style={{ backgroundColor: 'var(--border-primary)' }} />
          <div className="px-3 py-1.5 shrink-0">
            <button onClick={exportToCSV} className="flex items-center gap-1.5 px-2 py-1 text-sm text-text-secondary rounded hover:bg-[var(--hover-bg)] transition-colors">
              <LuDownload className="size-3.5" />
              Export
            </button>
          </div>
          <div className="h-4 w-px shrink-0" style={{ backgroundColor: 'var(--border-primary)' }} />
          <div className="px-3 py-1.5 shrink-0 flex items-center gap-1.5">
            <button
              className="flex items-center justify-center size-7 rounded hover:bg-[var(--hover-bg)] transition-colors disabled:opacity-30"
              disabled={page === 0}
              onClick={() => setPage(p => Math.max(0, p - 1))}
            >
              <LuChevronLeft className="size-3.5" style={{ color: 'var(--text-secondary)' }} />
            </button>
            <span className="text-xs text-text-secondary whitespace-nowrap">
              {totalPages > 0 ? `${page + 1} / ${totalPages}` : '-'}
            </span>
            <button
              className="flex items-center justify-center size-7 rounded hover:bg-[var(--hover-bg)] transition-colors disabled:opacity-30"
              disabled={page >= totalPages - 1}
              onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))}
            >
              <LuChevronRight className="size-3.5" style={{ color: 'var(--text-secondary)' }} />
            </button>
          </div>
        </div>
        <div className="flex-1 overflow-auto min-h-0 relative">
          <div className="min-w-fit flex flex-col">
            <div className="flex items-center h-9 border-b text-sm font-medium shrink-0 sticky top-0 z-10" style={{ borderColor: 'var(--border-primary)', backgroundColor: 'color-mix(in srgb, var(--bg-primary) 40%, var(--bg-secondary))' }}>
            <div className="w-[60px] shrink-0 h-full border-r border-l" style={{ borderColor: 'var(--border-primary)' }}></div>
            {columns.map((col, i) => {
              const isLast = i === columns.length - 1
              const isActive = sortColumn === col.key
              return (
                <button
                  key={col.key}
                  className={`flex items-center gap-2 ${col.width} shrink-0 px-4 h-full hover:bg-[var(--hover-bg)] transition-colors cursor-pointer ${!isLast ? 'border-r' : ''}`}
                  style={{ borderColor: 'var(--border-primary)' }}
                  onClick={() => handleSort(col.key)}
                >
                  <span className="text-text-primary">{col.label}</span>
                    <span className="ml-1 flex items-center">
                    {isActive && sortDirection === 'asc' ? (
                      <LuChevronUp className="size-3" style={{ color: 'var(--text-secondary)' }} />
                    ) : (
                      <LuChevronDown className="size-3" style={{ color: isActive ? 'var(--text-secondary)' : 'var(--border-primary)' }} />
                    )}
                  </span>
                </button>
              )
            })}
          </div>
          <div>
            {bodyRows}
          </div>
        </div>
        </div>
      </div>
    </div>
  )
}