import { useState } from 'react'
import { LuSearch, LuChevronDown, LuChevronRight } from 'react-icons/lu'

const healthStatuses = [
  { id: 'healthy', label: 'Healthy', count: 2, color: 'var(--success)' },
  { id: 'degraded', label: 'Degraded', count: 1, color: 'var(--warn)' },
  { id: 'down', label: 'Down', count: 1, color: 'var(--error)' },
]

const services = [
  { id: 'api', label: 'api', count: 2341 },
  { id: 'web', label: 'web', count: 1567 },
  { id: 'db', label: 'db', count: 892 },
  { id: 'worker', label: 'worker', count: 423 },
]

const environments = [
  { id: 'production', label: 'production', count: 4 },
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
              <span className="flex items-center gap-1.5 text-sm" style={{ color: item.color ?? 'var(--text-primary)' }}>
                {item.color && <span className="size-2 rounded-full" style={{ backgroundColor: item.color }} />}
                {item.label}
              </span>
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

export default function ServicesFilterSidebar() {
  const [openStates, setOpenStates] = useState<Record<string, boolean>>({
    health_status: true,
    service_name: false,
    environment: false,
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
          title="health_status"
          items={healthStatuses}
          open={openStates.health_status}
          onToggle={() => toggle('health_status')}
          checked={checked}
          onCheck={check}
        />

        <FilterSection
          title="service_name"
          items={services}
          open={openStates.service_name}
          onToggle={() => toggle('service_name')}
          checked={checked}
          onCheck={check}
        />

        <FilterSection
          title="environment"
          items={environments}
          open={openStates.environment}
          onToggle={() => toggle('environment')}
          checked={checked}
          onCheck={check}
          initialLimit={10}
        />
      </div>
    </div>
  )
}
