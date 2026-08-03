import { useState, useMemo } from 'react'
import { LuSearch, LuChevronDown, LuChevronRight } from 'react-icons/lu'
import type { FilterSectionItem, FilterSidebarProps } from '../types/index.ts'

// Versioned so a change to the default open/closed state (e.g. switching back
// to open-by-default) takes effect for everyone rather than reusing states
// saved under an older default.
const STORAGE_KEY = 'greplog:filterSidebar:open:v3'

function loadOpenStates(): Record<string, boolean> {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw) as Record<string, unknown>
    const result: Record<string, boolean> = {}
    for (const [key, value] of Object.entries(parsed)) {
      if (typeof value === 'boolean') result[key] = value
    }
    return result
  } catch {
    return {}
  }
}

function saveOpenStates(state: Record<string, boolean>) {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state))
  } catch {
    // storage unavailable — collapse persistence is best-effort
  }
}

function FilterSectionSkeleton() {
  return (
    <>
      {[0, 1, 2, 3].map((i) => (
        <div key={i} className="flex items-center gap-2 px-1 py-1">
          <div className="size-3.5 rounded animate-pulse" style={{ backgroundColor: 'var(--hover-bg)' }} />
          <div className="h-3.5 flex-1 rounded animate-pulse" style={{ backgroundColor: 'var(--hover-bg)' }} />
          <div className="h-3.5 w-8 rounded animate-pulse" style={{ backgroundColor: 'var(--hover-bg)' }} />
        </div>
      ))}
    </>
  )
}

function FilterSection({
  sectionId,
  title,
  items,
  open,
  onToggle,
  checked,
  onCheck,
  loading,
  initialLimit = 5,
}: {
  sectionId: string
  title: string
  items: FilterSectionItem[]
  open: boolean
  onToggle: () => void
  checked: Record<string, boolean>
  onCheck: (sectionId: string, id: string) => void
  loading: boolean
  initialLimit?: number
}) {
  const [expanded, setExpanded] = useState(false)
  const visibleItems = expanded ? items : items.slice(0, initialLimit)
  const hasMore = items.length > initialLimit

  return (
    <>
      <button
        className="flex w-full items-center justify-between px-3 py-2 text-sm hover:bg-[var(--hover-bg-subtle)] transition-colors"
        onClick={onToggle}
      >
        <span style={{ color: 'var(--text-primary)' }}>{title}</span>
        {open ? <LuChevronDown className="size-3.5" style={{ color: 'var(--text-secondary)' }} /> : <LuChevronRight className="size-3.5" style={{ color: 'var(--text-secondary)' }} />}
      </button>
      {open && (
        <div className="px-3 pb-2 flex flex-col gap-0.5">
          {loading ? (
            <FilterSectionSkeleton />
          ) : visibleItems.length === 0 ? (
            <div className="px-1 py-1 text-sm" style={{ color: 'var(--text-secondary)' }}>No Filters</div>
          ) : (
            <>
              {visibleItems.map((item) => (
                <label
                  key={item.id}
                  className="flex items-center gap-2 px-1 py-1 rounded hover:bg-[var(--hover-bg-subtle)] cursor-pointer transition-colors"
                >
                  <input
                    type="checkbox"
                    checked={checked[item.id] ?? false}
                    onChange={() => onCheck(sectionId, item.id)}
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
            </>
          )}
        </div>
      )}
      <div className="border-b" style={{ borderColor: 'var(--border-primary)' }} />
    </>
  )
}

export default function FilterSidebar({
  checked,
  onCheck,
  sections,
  loading = false,
  width = 280,
  searchPlaceholder = 'Search filters...',
}: FilterSidebarProps) {
  const [openStates, setOpenStates] = useState<Record<string, boolean>>(() => {
    const saved = loadOpenStates()
    const initial: Record<string, boolean> = {}
    for (const s of sections) {
      initial[s.id] = saved[s.id] ?? s.defaultOpen ?? false
    }
    return initial
  })
  const [searchTerm, setSearchTerm] = useState('')

  function toggle(section: string) {
    setOpenStates((prev) => {
      const next = { ...prev, [section]: !prev[section] }
      saveOpenStates(next)
      return next
    })
  }

  const filteredSections = useMemo(() => {
    if (!searchTerm.trim()) return sections
    const q = searchTerm.toLowerCase()
    return sections.map((s) => ({
      ...s,
      items: s.items.filter((i) => i.label.toLowerCase().includes(q)),
    }))
  }, [sections, searchTerm])

  return (
    <div
      className="border-r flex flex-col h-full"
      style={{
        width,
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
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            placeholder={searchPlaceholder}
            className="flex-1 text-xs bg-transparent outline-none"
            style={{ color: 'var(--text-primary)' }}
          />
        </div>
      </div>
      <div className="border-b shrink-0" style={{ borderColor: 'var(--border-primary)' }} />

      <div className="flex-1 overflow-y-auto">
        {filteredSections.map((section) => (
          <FilterSection
            key={section.id}
            sectionId={section.id}
            title={section.title}
            items={section.items}
            open={openStates[section.id] ?? false}
            onToggle={() => toggle(section.id)}
            checked={checked}
            onCheck={onCheck}
            loading={loading}
            initialLimit={section.initialLimit}
          />
        ))}
      </div>
    </div>
  )
}
