import { useState, useRef } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { LuChevronDown, LuChevronUp, LuCircleCheck, LuDownload, LuColumns2, LuRows3, LuClock, LuSignal, LuServer, LuMessageSquareText, LuTag, LuTimer, LuCode, LuLink, LuFile } from 'react-icons/lu'
import Dropdown from './Dropdown.tsx'

const columns = [
  { key: 'timestamp', label: 'timestamp', icon: LuClock, width: 'w-[180px]' },
  { key: 'level', label: 'level', icon: LuSignal, width: 'w-[100px]' },
  { key: 'service', label: 'service_name', icon: LuServer, width: 'w-[150px]' },
  { key: 'message', label: 'message', icon: LuMessageSquareText, width: 'w-[360px]' },
  { key: 'logger', label: 'logger_name', icon: LuTag, width: 'w-[150px]' },
  { key: 'correlationId', label: 'correlation_id', icon: LuLink, width: 'w-[150px]' },
  { key: 'file', label: 'file', icon: LuFile, width: 'w-[180px]' },
  { key: 'response', label: 'response', icon: LuTimer, width: 'w-[130px]' },
  { key: 'statusCode', label: 'status_code', icon: LuCode, width: 'w-[130px]' },
]

const levels = ['info', 'warn', 'error', 'debug'] as const

const mockData = Array.from({ length: 50000 }, (_, i) => ({
  id: i,
  timestamp: new Date(Date.now() - i * 60000).toISOString().replace('T', ' ').slice(0, 19),
  level: levels[i % levels.length],
  service: ['web', 'api', 'db', 'worker'][i % 4],
  message: [
    'Request processed successfully',
    'Cache miss for key user:1234',
    'Connection pool exhausted',
    'GET /api/users completed in 45ms',
    'Failed to connect to upstream',
    'Job queue processed 12 items',
    'Slow query detected (2.3s)',
    'Health check passed',
  ][i % 8],
  logger: ['http.server', 'db.connection', 'app.controller', 'background.worker'][i % 4],
  correlationId: `corr-${Math.random().toString(36).slice(2, 10)}`,
  file: ['src/http/server.ts', 'src/db/pool.ts', 'src/controllers/users.ts', 'src/workers/jobs.ts'][i % 4],
  response: `${Math.floor(Math.random() * 500) + 10}ms`,
  statusCode: [200, 201, 301, 400, 401, 403, 404, 500, 502, 503][i % 10],
}))

export default function LogsTable() {
  const [totalRows] = useState(1234)
  const [totalLogs] = useState(5678)
  const [seconds] = useState(0.3)
  const limits = ['500', '1k', '5k', '10k']
  const [limit, setLimit] = useState('500')
  const [columnsLayout, setColumnsLayout] = useState(true)
  const [sortColumn, setSortColumn] = useState<string | null>(null)
  const [sortDirection, setSortDirection] = useState<'asc' | 'desc'>('asc')
  const cardRef = useRef<HTMLDivElement>(null)

  const virtualizer = useVirtualizer({
    count: mockData.length,
    estimateSize: () => 28,
    getScrollElement: () => cardRef.current,
    overscan: 30,
  })

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
            <button className="flex items-center gap-1.5 px-2 py-1 text-sm text-text-secondary rounded hover:bg-[var(--hover-bg)] transition-colors">
              <LuDownload className="size-3.5" />
              Export
            </button>
          </div>
          <div className="h-4 w-px shrink-0" style={{ backgroundColor: 'var(--border-primary)' }} />
          <div className="px-3 py-1.5 shrink-0">
            <button
              className="flex items-center justify-center size-7 rounded hover:bg-[var(--hover-bg)] transition-colors"
              onClick={() => setColumnsLayout(!columnsLayout)}
              title={columnsLayout ? 'Switch to rows' : 'Switch to columns'}
            >
              {columnsLayout ? <LuColumns2 className="size-3.5 text-text-secondary" /> : <LuRows3 className="size-3.5 text-text-secondary" />}
            </button>
          </div>
        </div>
        <div ref={cardRef} className="flex-1 overflow-auto min-h-0 relative">
          <div className="min-w-fit flex flex-col">
            <div className="flex items-center h-8 border-b text-sm font-medium shrink-0 sticky top-0 z-10" style={{ borderColor: 'var(--border-primary)', backgroundColor: 'color-mix(in srgb, var(--bg-primary) 40%, var(--bg-secondary))' }}>
            <div className="w-[60px] shrink-0 h-full border-r" style={{ borderColor: 'var(--border-primary)' }}></div>
            {columns.map((col, i) => {
              const Icon = col.icon
              const isLast = i === columns.length - 1
              const isActive = sortColumn === col.key
              return (
                <button
                  key={col.key}
                  className={`flex items-center gap-1.5 ${col.width} shrink-0 px-3 h-full hover:bg-[var(--hover-bg)] transition-colors cursor-pointer ${!isLast ? 'border-r' : ''}`}
                  style={{ borderColor: 'var(--border-primary)' }}
                  onClick={() => handleSort(col.key)}
                >
                  <Icon className="size-3.5 shrink-0" style={{ color: 'var(--text-secondary)' }} />
                  <span className="text-text-primary">{col.label}</span>
                  <span className="ml-auto flex items-center">
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
          <div style={{ height: `${virtualizer.getTotalSize()}px`, position: 'relative', width: '100%' }}>
            {virtualizer.getVirtualItems().map((virtualItem) => {
              const row = mockData[virtualItem.index]
              const levelColor = row.level === 'error' ? 'var(--error)' : row.level === 'warn' ? 'var(--warn)' : row.level === 'info' ? 'var(--info)' : 'var(--text-secondary)'
              const statusColor = row.statusCode >= 500 ? 'var(--error)' : row.statusCode >= 400 ? 'var(--warn)' : row.statusCode >= 300 ? 'var(--info)' : 'var(--success)'
              return (
                <div
                  key={virtualItem.key}
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    width: '100%',
                    height: `${virtualItem.size}px`,
                    transform: `translateY(${virtualItem.start}px)`,
                    borderColor: 'var(--border-primary)',
                  }}
                  className="flex items-center border-b text-xs hover:bg-[var(--hover-bg-subtle)] transition-colors font-mono"
                >
                  <div className="w-[60px] shrink-0 h-full border-r flex items-center justify-center" style={{ borderColor: 'var(--border-primary)' }}>
                    <button className="text-xs text-[var(--accent)] hover:text-[var(--accent)] transition-colors cursor-pointer" onClick={() => {}}>View</button>
                  </div>
                  <div className="w-[180px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{row.timestamp}</div>
                  <div className="w-[100px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center" style={{ color: levelColor, borderColor: 'var(--border-primary)' }}>{row.level}</div>
                  <div className="w-[150px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{row.service}</div>
                  <div className="w-[360px] shrink-0 px-3 text-text-primary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{row.message}</div>
                  <div className="w-[150px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{row.logger}</div>
                  <div className="w-[150px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{row.correlationId}</div>
                  <div className="w-[180px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{row.file}</div>
                  <div className="w-[130px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{row.response}</div>
                  <div className="w-[130px] shrink-0 px-3 font-medium border-r h-full flex items-center" style={{ color: statusColor, borderColor: 'var(--border-primary)' }}>{row.statusCode}</div>
                </div>
              )
            })}
          </div>
        </div>
        </div>
      </div>
    </div>
  )
}
