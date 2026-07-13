import { useState } from 'react'
import { LuSearch, LuChevronDown, LuChevronRight } from 'react-icons/lu'

const statusGroups = [
  { id: 'success', label: 'Success', count: 1243 },
  { id: 'redirect', label: 'Redirect', count: 89 },
  { id: 'client_error', label: 'Client Error', count: 342 },
  { id: 'server_error', label: 'Server Error', count: 27 },
]

const statusCodes = [
  { id: '200', label: '200', count: 892 },
  { id: '201', label: '201', count: 351 },
  { id: '301', label: '301', count: 45 },
  { id: '400', label: '400', count: 156 },
  { id: '401', label: '401', count: 89 },
  { id: '403', label: '403', count: 42 },
  { id: '404', label: '404', count: 55 },
  { id: '500', label: '500', count: 19 },
  { id: '502', label: '502', count: 5 },
  { id: '503', label: '503', count: 3 },
]

const logLevels = [
  { id: 'error', label: 'Error', count: 27, color: 'var(--error)' },
  { id: 'warn', label: 'Warn', count: 156, color: 'var(--warn)' },
  { id: 'info', label: 'Info', count: 1243, color: 'var(--info)' },
  { id: 'debug', label: 'Debug', count: 3456, color: 'var(--text-secondary)' },
]

const services = [
  { id: 'web', label: 'web', count: 2341 },
  { id: 'api', label: 'api', count: 1567 },
  { id: 'db', label: 'db', count: 892 },
  { id: 'worker', label: 'worker', count: 423 },
]

function FilterSection({
  title,
  items,
  open,
  onToggle,
  checked,
  onCheck,
}: {
  title: string
  items: { id: string; label: string; count: number; color?: string }[]
  open: boolean
  onToggle: () => void
  checked: Record<string, boolean>
  onCheck: (id: string) => void
}) {
  return (
    <>
      <button
        className="flex items-center justify-between px-3 py-2 text-sm hover:bg-gray-50 transition-colors"
        onClick={onToggle}
      >
        <span style={{ color: 'var(--text-primary)' }}>{title}</span>
        {open ? <LuChevronDown className="size-3.5" style={{ color: 'var(--text-secondary)' }} /> : <LuChevronRight className="size-3.5" style={{ color: 'var(--text-secondary)' }} />}
      </button>
      {open && (
        <div className="px-3 pb-2 flex flex-col gap-0.5">
          {items.map((item) => (
            <label
              key={item.id}
              className="flex items-center gap-2 px-1 py-1 rounded hover:bg-gray-50 cursor-pointer transition-colors"
            >
              <input
                type="checkbox"
                checked={checked[item.id] ?? false}
                onChange={() => onCheck(item.id)}
                className="size-3.5 rounded border-gray-300"
              />
              <span className="text-sm" style={{ color: item.color ?? 'var(--text-primary)' }}>{item.label}</span>
              <span className="ml-auto text-sm" style={{ color: 'var(--text-secondary)' }}>{item.count}</span>
            </label>
          ))}
          <button className="text-sm text-left px-1 py-1 hover:opacity-80 transition-opacity" style={{ color: 'var(--accent)' }}>View more</button>
        </div>
      )}
      <div className="border-b" style={{ borderColor: 'var(--border-primary)' }} />
    </>
  )
}

export default function FilterSidebar() {
  const [openStates, setOpenStates] = useState<Record<string, boolean>>({
    status: true,
    codes: false,
    level: true,
    service: false,
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
          title="response_status"
          items={statusGroups}
          open={openStates.status}
          onToggle={() => toggle('status')}
          checked={checked}
          onCheck={check}
        />

        <FilterSection
          title="status_code"
          items={statusCodes}
          open={openStates.codes}
          onToggle={() => toggle('codes')}
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
          title="service_name"
          items={services}
          open={openStates.service}
          onToggle={() => toggle('service')}
          checked={checked}
          onCheck={check}
        />
      </div>
    </div>
  )
}
