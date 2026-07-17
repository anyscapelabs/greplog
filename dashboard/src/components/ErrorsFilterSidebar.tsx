import { useState } from 'react'
import { LuSearch, LuChevronDown, LuChevronRight } from 'react-icons/lu'

const errorStatuses = [
  { id: '500', label: '500 Internal Server Error', count: 19 },
  { id: '502', label: '502 Bad Gateway', count: 8 },
  { id: '503', label: '503 Service Unavailable', count: 5 },
  { id: '504', label: '504 Gateway Timeout', count: 3 },
  { id: '400', label: '400 Bad Request', count: 156 },
  { id: '401', label: '401 Unauthorized', count: 89 },
  { id: '403', label: '403 Forbidden', count: 42 },
  { id: '404', label: '404 Not Found', count: 55 },
  { id: '408', label: '408 Request Timeout', count: 12 },
  { id: '429', label: '429 Too Many Requests', count: 28 },
]

const errorTypes = [
  { id: 'timeout', label: 'Timeout', count: 34, color: 'var(--error)' },
  { id: 'connection', label: 'Connection Error', count: 23, color: 'var(--error)' },
  { id: 'validation', label: 'Validation Error', count: 67, color: 'var(--warn)' },
  { id: 'auth', label: 'Auth Error', count: 89, color: 'var(--warn)' },
  { id: 'runtime', label: 'Runtime Error', count: 45, color: 'var(--error)' },
  { id: 'db', label: 'Database Error', count: 18, color: 'var(--error)' },
]

const services = [
  { id: 'web', label: 'web', count: 234 },
  { id: 'api', label: 'api', count: 567 },
  { id: 'db', label: 'db', count: 89 },
  { id: 'worker', label: 'worker', count: 123 },
]

const logLevels = [
  { id: 'error', label: 'Error', count: 27, color: 'var(--error)' },
  { id: 'critical', label: 'Critical', count: 5, color: 'var(--error)' },
  { id: 'warn', label: 'Warn', count: 156, color: 'var(--warn)' },
]

function FilterSection({
  title,
  items,
  open,
  onToggle,
  checked,
  onCheck,
  initialLimit = 5,
}: {
  title: string
  items: { id: string; label: string; count: number; color?: string }[]
  open: boolean
  onToggle: () => void
  checked: Record<string, boolean>
  onCheck: (id: string) => void
  initialLimit?: number
}) {
  const [expanded, setExpanded] = useState(false)
  const visibleItems = expanded ? items : items.slice(0, initialLimit)
  const hasMore = items.length > initialLimit

  return (
    <>
      <button
        className="flex items-center justify-between px-3 py-2 text-sm hover:bg-[var(--hover-bg-subtle)] transition-colors"
        onClick={onToggle}
      >
        <span style={{ color: 'var(--text-primary)' }}>{title}</span>
        {open ? <LuChevronDown className="size-3.5" style={{ color: 'var(--text-secondary)' }} /> : <LuChevronRight className="size-3.5" style={{ color: 'var(--text-secondary)' }} />}
      </button>
      {open && (
        <div className="px-3 pb-2 flex flex-col gap-0.5">
          {visibleItems.map((item) => (
            <label
              key={item.id}
              className="flex items-center gap-2 px-1 py-1 rounded hover:bg-[var(--hover-bg-subtle)] cursor-pointer transition-colors"
            >
              <input
                type="checkbox"
                checked={checked[item.id] ?? false}
                onChange={() => onCheck(item.id)}
                className="size-3.5 rounded border-[var(--border-primary)]"
              />
              <span className="text-sm" style={{ color: item.color ?? 'var(--text-primary)' }}>{item.label}</span>
              <span className="ml-auto text-sm" style={{ color: 'var(--text-secondary)' }}>{item.count}</span>
            </label>
          ))}
          {hasMore && (
            <button
              className="text-sm text-left px-1 py-1 hover:opacity-80 transition-opacity"
              style={{ color: 'var(--accent)' }}
              onClick={() => setExpanded(!expanded)}
            >
              {expanded ? 'Show less' : 'View more'}
            </button>
          )}
        </div>
      )}
      <div className="border-b" style={{ borderColor: 'var(--border-primary)' }} />
    </>
  )
}

export default function ErrorsFilterSidebar() {
  const [openStates, setOpenStates] = useState<Record<string, boolean>>({
    service: true,
    type: true,
    level: false,
    status: false,
  })
  const [checked, setChecked] = useState<Record<string, boolean>>({})

  function toggle(section: string) {
    setOpenStates((prev) => ({ ...prev, [section]: !prev[section] }))
  }

  function check(id: string) {
    setChecked((prev) => ({ ...prev, [id]: !prev[id] }))
  }

  return (
    <div
      className="border-r flex flex-col h-full"
      style={{
        width: 280,
        backgroundColor: 'var(--bg-secondary)',
        borderColor: 'var(--border-primary)',
      }}
    >
      <div className="px-3 pt-3 pb-2">
        <span className="text-sm font-medium" style={{ color: 'var(--text-secondary)' }}>Filters</span>
      </div>
      <div className="px-3 pb-3">
        <div
          className="flex items-center px-2 py-1"
          style={{
            borderColor: 'var(--border-primary)',
            borderWidth: 1,
          }}
        >
          <LuSearch className="size-3.5 shrink-0 mr-1.5" style={{ color: 'var(--text-secondary)' }} />
          <input
            type="text"
            placeholder="Search filters..."
            className="flex-1 text-xs bg-transparent outline-none"
            style={{ color: 'var(--text-primary)' }}
          />
        </div>
      </div>
      <div className="border-b shrink-0" style={{ borderColor: 'var(--border-primary)' }} />

      <div className="flex-1 overflow-y-auto">
        <FilterSection
          title="service_name"
          items={services}
          open={openStates.service}
          onToggle={() => toggle('service')}
          checked={checked}
          onCheck={check}
        />

        <FilterSection
          title="error_type"
          items={errorTypes}
          open={openStates.type}
          onToggle={() => toggle('type')}
          checked={checked}
          onCheck={check}
        />

        <FilterSection
          title="log_level"
          items={logLevels}
          open={openStates.level}
          onToggle={() => toggle('level')}
          checked={checked}
          onCheck={check}
        />

        <FilterSection
          title="status_code"
          items={errorStatuses}
          open={openStates.status}
          onToggle={() => toggle('status')}
          checked={checked}
          onCheck={check}
        />
      </div>
    </div>
  )
}
