import { useEffect, useMemo, useRef, useState } from 'react'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import { LuChevronDown, LuChevronRight, LuCopy, LuDownload, LuFilter, LuChevronLeft, LuChevronRight as LuNext } from 'react-icons/lu'

type SpanRow = {
  span_id: string
  parent_span_id: string | null
  trace_id: string
  service: string
  operation: string
  duration_ms: number
  start_offset_ms: number
  depth: number
}

const MOCK_TRACE_ID = 'trc_8f2a1c'
const MOCK_TOTAL_MS = 134.47

const MOCK_SPANS: SpanRow[] = [
  { trace_id: MOCK_TRACE_ID, span_id: 'root', parent_span_id: null, service: 'mythical-requester', operation: 'requester', duration_ms: 105.52, start_offset_ms: 0, depth: 0 },
  { trace_id: MOCK_TRACE_ID, span_id: 's1', parent_span_id: 'root', service: 'mythical-requester', operation: 'requester', duration_ms: 69.86, start_offset_ms: 0.5, depth: 1 },
  { trace_id: MOCK_TRACE_ID, span_id: 's2', parent_span_id: 'root', service: 'mythical-server', operation: 'GET', duration_ms: 58.28, start_offset_ms: 2, depth: 1 },
  { trace_id: MOCK_TRACE_ID, span_id: 's3', parent_span_id: 's2', service: 'mythical-server', operation: 'query', duration_ms: 35.33, start_offset_ms: 4, depth: 2 },
  { trace_id: MOCK_TRACE_ID, span_id: 's4', parent_span_id: 's2', service: 'mythical-server', operation: 'query', duration_ms: 28.93, start_offset_ms: 5, depth: 2 },
  { trace_id: MOCK_TRACE_ID, span_id: 's5', parent_span_id: 's2', service: 'mythical-server', operation: 'query', duration_ms: 3.58, start_offset_ms: 6, depth: 2 },
  { trace_id: MOCK_TRACE_ID, span_id: 's6', parent_span_id: 's2', service: 'mythical-server', operation: 'process', duration_ms: 44.67, start_offset_ms: 10, depth: 2 },
  { trace_id: MOCK_TRACE_ID, span_id: 's7', parent_span_id: 's2', service: 'mythical-server', operation: 'validate', duration_ms: 38.14, start_offset_ms: 12, depth: 2 },
  { trace_id: MOCK_TRACE_ID, span_id: 's8', parent_span_id: 's2', service: 'mythical-server', operation: 'cache', duration_ms: 5.33, start_offset_ms: 60, depth: 2 },
  { trace_id: MOCK_TRACE_ID, span_id: 's9', parent_span_id: 's2', service: 'mythical-server', operation: 'cache', duration_ms: 3.54, start_offset_ms: 62, depth: 3 },
  { trace_id: MOCK_TRACE_ID, span_id: 's10', parent_span_id: 's2', service: 'mythical-server', operation: 'cache', duration_ms: 1.95, start_offset_ms: 63, depth: 3 },
  { trace_id: MOCK_TRACE_ID, span_id: 's11', parent_span_id: 'root', service: 'mythical-requester', operation: 'requester', duration_ms: 0.33, start_offset_ms: 70, depth: 1 },
  { trace_id: MOCK_TRACE_ID, span_id: 's12', parent_span_id: 'root', service: 'mythical-requester', operation: 'requester', duration_ms: 21.63, start_offset_ms: 75, depth: 1 },
  { trace_id: MOCK_TRACE_ID, span_id: 's13', parent_span_id: 'root', service: 'mythical-requester', operation: 'requester', duration_ms: 2.27, start_offset_ms: 80, depth: 1 },
  { trace_id: MOCK_TRACE_ID, span_id: 's14', parent_span_id: 'root', service: 'mythical-requester', operation: 'requester', duration_ms: 1.70, start_offset_ms: 82, depth: 1 },
  { trace_id: MOCK_TRACE_ID, span_id: 'db1', parent_span_id: 's2', service: 'mythical-server', operation: 'db.query', duration_ms: 0.77, start_offset_ms: 90, depth: 2 },
  { trace_id: MOCK_TRACE_ID, span_id: 'db2', parent_span_id: 'db1', service: 'mythical-server', operation: 'db.query', duration_ms: 40.47, start_offset_ms: 95, depth: 3 },
]

const SERVICE_COLOR: Record<string, string> = {
  'mythical-requester': '#a06bff',
  'mythical-server': '#38bdf8',
}

function barColor(service: string, depth: number): string {
  const base = SERVICE_COLOR[service] ?? '#a06bff'
  if (depth === 0) return base
  return service === 'mythical-requester' ? 'rgba(160,107,255,0.65)' : 'rgba(56,189,248,0.55)'
}

function buildTree(spans: SpanRow[]): Map<string, SpanRow[]> {
  const map = new Map<string, SpanRow[]>()
  for (const s of spans) {
    const key = s.parent_span_id ?? '__root__'
    if (!map.has(key)) map.set(key, [])
    map.get(key)!.push(s)
  }
  return map
}

export default function TraceWaterfall({ spans = MOCK_SPANS, traceId = MOCK_TRACE_ID, totalMs = MOCK_TOTAL_MS }: { spans?: SpanRow[], traceId?: string, totalMs?: number }) {
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set())
  const [filterOpen, setFilterOpen] = useState(false)
  const [filterText, setFilterText] = useState('')
  const [selectedIdx, setSelectedIdx] = useState(0)
  const [copied, setCopied] = useState(false)
  const minimapRef = useRef<HTMLDivElement>(null)
  const minimapPlotRef = useRef<uPlot | null>(null)

  const filteredSpans = useMemo(() => {
    if (!filterText.trim()) return spans
    const q = filterText.toLowerCase()
    return spans.filter((s) => s.service.toLowerCase().includes(q) || s.operation.toLowerCase().includes(q))
  }, [spans, filterText])

  const tree = useMemo(() => buildTree(filteredSpans), [filteredSpans])

  const visibleSpans = useMemo(() => {
    const out: SpanRow[] = []
    const walk = (parentId: string | null) => {
      const key = parentId ?? '__root__'
      const children = tree.get(key) ?? []
      for (const child of children) {
        out.push(child)
        if (!collapsed.has(child.span_id)) walk(child.span_id)
      }
    }
    walk(null)
    return out
  }, [tree, collapsed])

  const ticks = [0, 33.62, 67.24, 100.86, 134.47]

  const toggleCollapse = (id: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(traceId)
      setCopied(true)
      setTimeout(() => setCopied(false), 1200)
    } catch {}
  }

  const handleExport = () => {
    const blob = new Blob([JSON.stringify(spans, null, 2)], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `${traceId}.json`
    a.click()
    URL.revokeObjectURL(url)
  }

  const handlePrev = () => setSelectedIdx((i) => (i - 1 + visibleSpans.length) % visibleSpans.length)
  const handleNext = () => setSelectedIdx((i) => (i + 1) % visibleSpans.length)

  useEffect(() => {
    const el = minimapRef.current
    if (!el) return
    const xs = filteredSpans.map((s) => s.start_offset_ms)
    const ys = filteredSpans.map((s) => s.duration_ms)
    const data: [number[], number[]] = [xs, ys]
    const opts: uPlot.Options = {
      width: el.clientWidth,
      height: 56,
      padding: [4, 0, 0, 0],
      cursor: { show: false },
      legend: { show: false },
      axes: [{ show: false }, { show: false }],
      scales: { x: { time: false, range: [0, totalMs] } },
      series: [
        {},
        {
          label: 'duration',
          fill: 'rgba(160,107,255,0.55)',
          stroke: '#a06bff',
          points: { show: false },
          paths: uPlot.paths.bars!({ size: [0.7, 100] }),
        },
      ],
    }
    const plot = new uPlot(opts, data, el)
    minimapPlotRef.current = plot
    const ro = new ResizeObserver(() => {
      if (minimapPlotRef.current && el) minimapPlotRef.current.setSize({ width: el.clientWidth, height: 56 })
    })
    ro.observe(el)
    return () => {
      ro.disconnect()
      plot.destroy()
      minimapPlotRef.current = null
    }
  }, [filteredSpans, totalMs])

  const hasChildren = (id: string) => (tree.get(id)?.length ?? 0) > 0

  return (
    <div className="overflow-hidden rounded border border-zinc-800 bg-zinc-950 text-sm">
      <div className="flex items-center justify-between border-b border-zinc-800 bg-zinc-900 px-3 py-2">
        <div className="min-w-0">
          <div className="truncate text-sm font-medium text-zinc-100">mythical-requester: requester <span className="font-mono text-xs font-normal text-zinc-400">{totalMs}ms</span></div>
          <div className="font-mono text-xs text-zinc-400">2023-07-20 14:10:38.703 <span className="ml-2 rounded bg-[#a06bff] px-1.5 py-0.5 text-[10px] font-medium text-white">GET</span> <span className="text-zinc-300">owlbear</span></div>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <button type="button" onClick={handleCopy} className="inline-flex items-center gap-1 rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-xs text-zinc-200 hover:bg-zinc-700">
            <LuCopy className="h-3 w-3" /> {copied ? 'Copied' : 'Trace ID'}
          </button>
          <button type="button" onClick={handleExport} className="inline-flex items-center gap-1 rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-xs text-zinc-200 hover:bg-zinc-700">
            <LuDownload className="h-3 w-3" /> Export
          </button>
        </div>
      </div>

      <div className="flex items-center justify-between border-b border-zinc-800 bg-zinc-900 px-3 py-1.5 text-sm">
        <button type="button" onClick={() => setFilterOpen((v) => !v)} className="inline-flex items-center gap-1 text-sm text-zinc-300 hover:text-zinc-100">
          <LuFilter className="h-3 w-3" /> Span Filters
          <span className="ml-1 flex h-3 w-3 items-center justify-center rounded-full border border-zinc-600 text-[10px]">?</span>
        </button>
        <div className="flex items-center gap-2 text-sm text-zinc-400">
          <span className="text-sm text-zinc-300">{visibleSpans.length} spans</span>
          <div className="flex overflow-hidden rounded border border-zinc-700 text-xs">
            <button type="button" onClick={handlePrev} className="inline-flex items-center gap-1 bg-zinc-800 px-2 py-1 text-zinc-200 hover:bg-zinc-700"><LuChevronLeft className="h-3 w-3" /> Prev</button>
            <button type="button" onClick={handleNext} className="inline-flex items-center gap-1 border-l border-zinc-700 bg-zinc-800 px-2 py-1 text-zinc-200 hover:bg-zinc-700">Next <LuNext className="h-3 w-3" /></button>
          </div>
        </div>
      </div>

      {filterOpen && (
        <div className="border-b border-zinc-800 bg-zinc-900 px-3 py-2">
          <input
            autoFocus
            value={filterText}
            onChange={(e) => setFilterText(e.target.value)}
            placeholder="Filter by service or operation (e.g. mythical-server)"
            className="w-full rounded border border-zinc-700 bg-zinc-950 px-2 py-1 text-sm text-zinc-200 placeholder:text-zinc-500 focus:border-[#a06bff] focus:outline-none"
          />
        </div>
      )}

      <div className="border-b border-zinc-800 bg-zinc-950">
        <div className="flex">
          <div className="w-[38%] shrink-0" />
          <div className="flex-1 px-2 py-2">
            <div ref={minimapRef} className="h-14 w-full overflow-hidden" />
          </div>
        </div>
        <div className="flex border-t border-zinc-800">
          <div className="w-[38%] shrink-0 border-r border-zinc-800 bg-zinc-900 px-2 py-1 text-xs text-zinc-500">Service & Operation</div>
          <div className="flex flex-1 divide-x divide-zinc-800/50 font-mono text-[11px] text-zinc-500">
            {ticks.map((t) => (
              <div key={t} className="flex-1 bg-zinc-900 px-1 py-1 text-center">{t === 0 ? '0µs' : `${t}ms`}</div>
            ))}
          </div>
        </div>
      </div>

      <div className="max-h-[640px] min-h-[360px] overflow-auto">
        <table className="w-full table-fixed border-collapse text-sm">
          <colgroup>
            <col style={{ width: '38%' }} />
            <col style={{ width: '62%' }} />
          </colgroup>
          <tbody>
            {visibleSpans.map((span, idx) => {
              const collapsible = hasChildren(span.span_id)
              const isCollapsed = collapsed.has(span.span_id)
              const isSelected = idx === selectedIdx
              const left = (span.start_offset_ms / totalMs) * 100
              const width = Math.max((span.duration_ms / totalMs) * 100, 0.8)
              const clampedLeft = Math.min(left, 100 - width)
              const clampedWidth = Math.min(width, 100 - clampedLeft)
              return (
                <tr key={span.span_id} className={`border-b border-zinc-800 ${isSelected ? 'bg-[#a06bff]/10' : 'hover:bg-zinc-800/40'}`}>
                  <td className="border-r border-zinc-800 px-3 py-1">
                    <button
                      type="button"
                      onClick={() => (collapsible ? toggleCollapse(span.span_id) : setSelectedIdx(idx))}
                      className="flex w-full items-center gap-1 text-left"
                      style={{ paddingLeft: `${span.depth * 14}px` }}
                    >
                      <span className="flex h-4 w-4 shrink-0 items-center justify-center text-zinc-500">
                        {collapsible ? (isCollapsed ? <LuChevronRight className="h-3 w-3" /> : <LuChevronDown className="h-3 w-3" />) : <span className="h-3 w-3" />}
                      </span>
                      <span className="h-3 w-[3px] shrink-0 rounded-sm" style={{ background: barColor(span.service, span.depth) }} />
                      <span className="truncate text-sm text-zinc-300">{span.service}</span>
                      <span className="ml-1 hidden truncate text-sm text-zinc-500 xl:inline">{span.operation}</span>
                    </button>
                  </td>
                  <td className="relative overflow-hidden px-1 py-0.5">
                    <div className="absolute inset-y-0 flex items-center" style={{ left: `calc(${clampedLeft}% + 4px)`, width: `calc(${clampedWidth}% - 4px)` }}>
                      <div className="h-4 w-full rounded-sm" style={{ background: barColor(span.service, span.depth), opacity: span.depth === 0 ? 0.95 : 0.7 }} />
                    </div>
                    {ticks.slice(1).map((t) => (
                      <div key={t} className="pointer-events-none absolute inset-y-0 w-px bg-zinc-800/60" style={{ left: `${(t / totalMs) * 100}%` }} />
                    ))}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </div>
  )
}

export { MOCK_SPANS, MOCK_TRACE_ID, MOCK_TOTAL_MS }
export type { SpanRow }
