import { useRef, useState } from 'react'
import {
  LuArrowLeftToLine,
  LuCheck,
  LuChevronDown,
  LuChevronRight,
} from 'react-icons/lu'
import type { QueryRow } from '../../api/logs'
import EmptyIcon from '../icons/EmptyIcon'

const MIN_FILTERS_WIDTH = 260
const DEFAULT_FILTERS_WIDTH = 256

interface DragState {
  start: number
  base: number
}

interface FacetItem {
  label: string
  count: number
}

interface FilterFacets {
  [section: string]: FacetItem[]
}

interface FiltersSidebarProps {
  facets?: QueryRow[]
  /** Wire-named facets currently filtering the view, e.g. { level: 'ERROR' }. */
  active?: Record<string, string>
  onFilterSelect?: (queryAddition: string) => void
}

function buildFacetSections(rows: QueryRow[]): FilterFacets {
  const severity = new Map<string, number>()
  const service = new Map<string, number>()
  for (const row of rows) {
    const level = String(row.level ?? '').trim()
    if (level) {
      severity.set(level, (severity.get(level) ?? 0) + (Number(row.count) || 1))
    }
    const svc = String(row.service ?? '').trim()
    if (svc) {
      service.set(svc, (service.get(svc) ?? 0) + (Number(row.count) || 1))
    }
  }
  const toItems = (map: Map<string, number>): FacetItem[] =>
    [...map.entries()]
      .map(([label, count]) => ({ label, count }))
      .sort((a, b) => b.count - a.count)
  const sections: FilterFacets = {}
  const severityItems = toItems(severity)
  const serviceItems = toItems(service)
  if (severityItems.length) sections.Severity = severityItems
  if (serviceItems.length) sections.Service = serviceItems
  return sections
}

function FiltersSidebar({ facets, active, onFilterSelect }: FiltersSidebarProps) {
  const [collapsed, setCollapsed] = useState(false)
  const [width, setWidth] = useState(DEFAULT_FILTERS_WIDTH)
  const [search, setSearch] = useState('')
  const [expanded, setExpanded] = useState<Record<string, boolean>>({
    Severity: true,
    Service: true,
  })
  const asideRef = useRef<HTMLElement>(null)
  const dragRef = useRef<DragState | null>(null)

  const stopDrag = () => {
    dragRef.current = null
    window.removeEventListener('pointermove', onPointerMove)
    window.removeEventListener('pointerup', stopDrag)
  }

  const onPointerMove = (event: PointerEvent) => {
    const state = dragRef.current
    if (!state) return
    const next = state.base + (event.clientX - state.start)
    if (next < MIN_FILTERS_WIDTH) {
      setCollapsed(true)
      stopDrag()
    } else {
      setWidth(Math.round(next))
    }
  }

  const startDrag = (event: React.PointerEvent) => {
    event.preventDefault()
    const aside = asideRef.current
    if (!aside) return
    dragRef.current = {
      start: event.clientX,
      base: aside.getBoundingClientRect().width,
    }
    window.addEventListener('pointermove', onPointerMove)
    window.addEventListener('pointerup', stopDrag)
  }

  const toggleSection = (section: string) => {
    setExpanded((prev) => ({ ...prev, [section]: !prev[section] }))
  }

  const handleSelect = (section: string, label: string) => {
    onFilterSelect?.(`${section.toLowerCase()}='${label}'`)
  }

  const isActive = (section: string, label: string): boolean => {
    const wireKey = section === 'Severity' ? 'level' : 'service'
    return active?.[wireKey] === label
  }

  const sections = buildFacetSections(facets ?? [])
  const filteredSections = Object.entries(sections).filter(
    ([section, items]) =>
      !search ||
      section.toLowerCase().includes(search.toLowerCase()) ||
      items.some((item) =>
        item.label.toLowerCase().includes(search.toLowerCase()),
      ),
  )

  return (
    <aside
      ref={asideRef}
      style={collapsed ? { width: '2.5rem' } : { width }}
      className="relative flex shrink-0 flex-col overflow-hidden border-r border-zinc-800 bg-zinc-950"
    >
      <div
        className={`flex shrink-0 items-center border-b border-zinc-800 p-2 ${
          collapsed ? 'justify-center' : 'justify-between'
        }`}
      >
        {!collapsed && (
          <h2 className="text-sm font-medium text-zinc-100">Fields</h2>
        )}
        <button
          type="button"
          onClick={() => setCollapsed((value) => !value)}
          className="cursor-pointer text-zinc-500 transition-colors hover:text-zinc-300"
        >
          <LuArrowLeftToLine
            className={`h-4 w-4 transition-transform ${
              collapsed ? 'rotate-180' : ''
            }`}
          />
        </button>
      </div>
      {!collapsed && (
        <>
          <div className="shrink-0 p-2">
            <input
              type="text"
              placeholder="Search fields and values"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-full rounded-md bg-zinc-900 px-2.5 py-1.5 text-sm font-medium text-zinc-300 placeholder-zinc-500 outline-none focus:bg-zinc-800"
            />
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {filteredSections.length === 0 && (
              <div className="flex flex-col items-center gap-2 px-4 py-10 text-center">
                <EmptyIcon className="h-14 w-14 text-zinc-600" />
                <p className="text-base font-medium text-zinc-400">No Data Found</p>
              </div>
            )}
            {filteredSections.map(([section, items]) => {
              const isOpen = expanded[section] ?? false
              return (
                <div key={section} className="border-b border-zinc-800">
                  <div
                    onClick={() => toggleSection(section)}
                    className="flex select-none cursor-pointer items-center justify-between p-2 text-sm font-medium text-zinc-400 hover:bg-zinc-800"
                  >
                    <div className="flex items-center">
                      {isOpen ? (
                        <LuChevronDown className="mr-1 h-3.5 w-3.5 text-zinc-500" />
                      ) : (
                        <LuChevronRight className="mr-1 h-3.5 w-3.5 text-zinc-500" />
                      )}
                      <span className="font-medium text-blue-500">
                        {section}
                      </span>
                    </div>
                  </div>
                  {isOpen && (
                    <div className="pb-1">
                      {items.map((item) => {
                        const selected = isActive(section, item.label)
                        return (
                          <label
                            key={item.label}
                            title={selected ? 'Uncheck to remove this filter' : undefined}
                            className="flex cursor-pointer items-center justify-between gap-2 px-3 py-2 text-sm font-medium text-zinc-300 transition-colors hover:bg-zinc-800 hover:text-white"
                          >
                            <span className="flex min-w-0 items-center gap-2">
                              <span className="relative flex h-3.5 w-3.5 shrink-0 items-center justify-center">
                                <input
                                  type="checkbox"
                                  checked={selected}
                                  onChange={() => handleSelect(section, item.label)}
                                  className={`h-3.5 w-3.5 shrink-0 cursor-pointer appearance-none rounded-[3px] border transition-colors ${
                                    selected
                                      ? 'border-blue-500 bg-blue-500'
                                      : 'border-zinc-600 bg-zinc-900 hover:border-zinc-400'
                                  }`}
                                />
                                {selected && (
                                  <LuCheck className="pointer-events-none absolute h-3 w-3 text-zinc-950" />
                                )}
                              </span>
                              <span className="truncate">{item.label}</span>
                            </span>
                            <span className="text-xs font-medium text-zinc-500">
                              {item.count}
                            </span>
                          </label>
                        )
                      })}
                    </div>
                  )}
                </div>
              )
            })}
          </div>
        </>
      )}
      {!collapsed && (
        <div
          onPointerDown={startDrag}
          className="absolute inset-y-0 right-0 w-1.5 cursor-col-resize bg-transparent transition-colors hover:bg-zinc-700"
        />
      )}
    </aside>
  )
}

export default FiltersSidebar