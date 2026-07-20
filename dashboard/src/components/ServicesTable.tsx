import { useState, useEffect, useMemo } from 'react'
import { LuChevronDown, LuChevronUp, LuCircleCheck, LuDownload, LuChevronLeft, LuChevronRight } from 'react-icons/lu'
import Dropdown from './Dropdown.tsx'

const columns = [
  { key: 'service', label: 'Service', width: 'w-[180px]' },
  { key: 'status', label: 'Status', width: 'w-[120px]' },
  { key: 'uptime', label: 'Uptime', width: 'w-[110px]' },
  { key: 'requests', label: 'Requests', width: 'w-[140px]' },
  { key: 'errorRate', label: 'Error Rate', width: 'w-[130px]' },
  { key: 'avgLatency', label: 'Avg Latency', width: 'w-[130px]' },
  { key: 'p95', label: 'P95', width: 'w-[120px]' },
  { key: 'p99', label: 'P99', width: 'w-[120px]' },
  { key: 'lastSeen', label: 'Last Seen', width: 'w-[170px]' },
]

const statusColors: Record<string, string> = {
  healthy: 'var(--success)',
  degraded: 'var(--warn)',
  down: 'var(--error)',
}

const mockData = Array.from({ length: 50000 }, (_, i) => {
  const services = ['api', 'web', 'db', 'worker']
  const statuses = ['healthy', 'healthy', 'healthy', 'degraded', 'down'] as const
  const name = services[i % services.length]
  const status = statuses[i % statuses.length]
  const uptime = status === 'healthy' ? (99 + Math.random() * 0.9).toFixed(1) : status === 'degraded' ? (90 + Math.random() * 9).toFixed(1) : (50 + Math.random() * 39).toFixed(1)
  const reqBase = status === 'healthy' ? 1000000 : status === 'degraded' ? 450000 : 120000
  const requests = Math.floor(reqBase + (Math.random() - 0.5) * reqBase * 0.4)
  const errorRate = status === 'healthy' ? (Math.random() * 0.9).toFixed(1) : status === 'degraded' ? (1 + Math.random() * 4).toFixed(1) : (5 + Math.random() * 10).toFixed(1)
  const avgLatency = status === 'healthy' ? Math.floor(20 + Math.random() * 80) : status === 'degraded' ? Math.floor(100 + Math.random() * 400) : Math.floor(500 + Math.random() * 500)
  const p95Latency = Math.floor(avgLatency * (1.5 + Math.random() * 2))
  const p99Latency = Math.floor(p95Latency * (1.2 + Math.random() * 1.5))
  const lastSeen = ['2m ago', '1m ago', '5m ago', '30s ago', '3m ago'][i % 5]

  return {
    id: i,
    service: name,
    status,
    uptime: `${uptime}%`,
    requests: requests >= 1000000 ? `${(requests / 1000000).toFixed(1)}M` : requests >= 1000 ? `${(requests / 1000).toFixed(0)}k` : String(requests),
    errorRate: `${errorRate}%`,
    avgLatency: `${avgLatency}ms`,
    p95: `${p95Latency}ms`,
    p99: `${p99Latency}ms`,
    lastSeen,
  }
})

interface ServicesTableProps {
  filteredServices?: string[]
  onView?: (row: any) => void
}

export default function ServicesTable({ filteredServices, onView }: ServicesTableProps) {
  const [totalRows] = useState(1234)
  const [totalLogs] = useState(5678)
  const [seconds] = useState(0.3)
  const limits = ['500', '1k', '5k', '10k']
  const [limit, setLimit] = useState('500')
  const [sortColumn, setSortColumn] = useState<string | null>(null)
  const [sortDirection, setSortDirection] = useState<'asc' | 'desc'>('asc')
  const [page, setPage] = useState(0)

  const data = filteredServices
    ? mockData.filter((row) => filteredServices.includes(row.service))
    : mockData

  const sortedData = useMemo(() => {
    if (!sortColumn) return data
    return [...data].sort((a, b) => {
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
  }, [data, sortColumn, sortDirection])

  const parsedLimit = limit === 'All' ? sortedData.length : parseInt(limit.replace('k', '000'))
  const displayData = useMemo(() => sortedData.slice(0, parsedLimit), [sortedData, parsedLimit])

  const pageSize = limit === 'All' ? displayData.length : parseInt(limit.replace('k', '000'))
  const totalPages = Math.ceil(displayData.length / pageSize)
  const pageData = useMemo(() => displayData.slice(page * pageSize, (page + 1) * pageSize), [displayData, page, pageSize])

  const bodyRows = useMemo(() => pageData.map((row) => (
    <div
      key={row.id}
      className="flex items-center border-b text-sm py-1 hover:bg-[var(--hover-bg-subtle)] transition-colors font-mono"
    >
      <div className="w-[60px] shrink-0 h-full border-r border-l flex items-center justify-center" style={{ borderColor: 'var(--border-primary)' }}>
        <button className="text-sm text-[var(--accent)] hover:text-[var(--accent)] transition-colors cursor-pointer" onClick={() => onView?.(row)}>View</button>
      </div>
      <div className="w-[180px] shrink-0 px-3 truncate border-r h-full flex items-center gap-1.5" style={{ borderColor: 'var(--border-primary)' }}>
        <span className="size-2 rounded-full shrink-0" style={{ backgroundColor: statusColors[row.status] }} />
        <span className="text-text-primary whitespace-nowrap">{row.service}</span>
      </div>
      <div className="w-[120px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center whitespace-nowrap" style={{ color: statusColors[row.status], borderColor: 'var(--border-primary)' }}>{row.status === 'healthy' ? 'Healthy' : row.status === 'degraded' ? 'Degraded' : 'Down'}</div>
      <div className="w-[110px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center whitespace-nowrap" style={{ borderColor: 'var(--border-primary)' }}>{row.uptime}</div>
      <div className="w-[140px] shrink-0 px-3 text-text-primary truncate border-r h-full flex items-center whitespace-nowrap" style={{ borderColor: 'var(--border-primary)' }}>{row.requests}</div>
      <div className="w-[130px] shrink-0 px-3 truncate border-r h-full flex items-center whitespace-nowrap" style={{ color: row.errorRate && parseFloat(row.errorRate) > 5 ? 'var(--error)' : parseFloat(row.errorRate) > 1 ? 'var(--warn)' : 'var(--text-secondary)', borderColor: 'var(--border-primary)' }}>{row.errorRate}</div>
      <div className="w-[130px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center whitespace-nowrap" style={{ borderColor: 'var(--border-primary)' }}>{row.avgLatency}</div>
      <div className="w-[120px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center whitespace-nowrap" style={{ borderColor: 'var(--border-primary)' }}>{row.p95}</div>
      <div className="w-[120px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center whitespace-nowrap" style={{ borderColor: 'var(--border-primary)' }}>{row.p99}</div>
      <div className="w-[170px] shrink-0 px-3 text-text-secondary truncate h-full flex items-center whitespace-nowrap" style={{ borderColor: 'var(--border-primary)' }}>{row.lastSeen}</div>
    </div>
  )), [pageData, onView])

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
    a.href = url
    a.download = 'services-export.csv'
    a.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="flex-1 min-h-0 min-w-0 overflow-hidden px-2 pt-1.5 pb-2">
      <div className="h-full w-full border flex flex-col min-w-0 overflow-hidden" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
        <div className="flex items-center border-b shrink-0 z-10" style={{ borderColor: 'var(--border-primary)', backgroundColor: 'var(--bg-secondary)' }}>
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
              <div className="w-[60px] shrink-0 h-full border-r border-l flex items-center justify-center" style={{ borderColor: 'var(--border-primary)' }}>
                <span className="text-xs" style={{ color: 'var(--text-secondary)' }}>View</span>
              </div>
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
                    <span className="text-text-primary whitespace-nowrap">{col.label}</span>
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
            <div style={{ position: 'relative', width: '100%' }}>
              {bodyRows}
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
