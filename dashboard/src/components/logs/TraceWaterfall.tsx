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
  'mythical-requester': '#0ea5e9',
  'mythical-server': '#bae6fd',
}

function barColor(service: string): string {
  return SERVICE_COLOR[service] ?? '#38bdf8'
}

export default function TraceWaterfall({ spans = MOCK_SPANS, totalMs = MOCK_TOTAL_MS }: { spans?: SpanRow[], traceId?: string, totalMs?: number }) {
  const ticks = [0, 33.62, 67.24, 100.86, 134.47]

  return (
    <div className="overflow-hidden rounded border border-zinc-800 bg-[#0f1419] text-sm">
      <div className="flex items-center justify-between border-b border-zinc-800 bg-[#1a232e] px-3 py-2">
        <div className="min-w-0">
          <div className="truncate text-sm font-medium text-zinc-100">mythical-requester: requester <span className="font-mono text-xs font-normal text-zinc-400">{totalMs}ms</span></div>
          <div className="font-mono text-xs text-zinc-400">2023-07-20 14:10:38.703 <span className="ml-2 rounded bg-sky-600 px-1.5 py-0.5 text-[10px] font-medium text-white">GET</span> <span className="text-zinc-300">owlbear</span></div>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <button type="button" className="rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-xs text-zinc-300">Trace ID</button>
          <button type="button" className="rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-xs text-zinc-300">Export</button>
        </div>
      </div>

      <div className="flex items-center justify-between border-b border-zinc-800 bg-[#111a23] px-3 py-1.5 text-xs">
        <button type="button" className="flex items-center gap-1 text-zinc-300">› Span Filters <span className="ml-1 flex h-3 w-3 items-center justify-center rounded-full border border-zinc-600 text-[10px]">?</span></button>
        <div className="flex items-center gap-2 text-zinc-400">
          <span className="text-zinc-300">{spans.length} spans</span>
          <div className="flex overflow-hidden rounded border border-zinc-700 text-[11px]">
            <button type="button" className="bg-zinc-800 px-2 py-0.5 text-zinc-300">Prev</button>
            <button type="button" className="border-l border-zinc-700 bg-zinc-800 px-2 py-0.5 text-zinc-300">Next</button>
          </div>
        </div>
      </div>

      <div className="relative h-12 border-b border-zinc-800 bg-[#0f1419] px-3 py-2">
        <div className="relative h-6 w-full">
          {ticks.map((t) => (
            <span key={t} className="absolute top-0 -translate-x-1/2 font-mono text-[10px] text-zinc-500" style={{ left: `${(t / totalMs) * 100}%` }}>{t === 0 ? '0µs' : `${t}ms`}</span>
          ))}
          <div className="absolute inset-x-0 top-3 h-[1px] bg-zinc-800" />
          <div className="absolute inset-x-0 top-3">
            {spans.slice(0, 8).map((s) => {
              const left = (s.start_offset_ms / totalMs) * 100
              const width = Math.max((s.duration_ms / totalMs) * 100, 0.6)
              return <div key={s.span_id} className="absolute h-[3px] rounded-sm opacity-70" style={{ left: `${left}%`, width: `${width}%`, top: `${(s.depth * 3)}px`, background: barColor(s.service) }} />
            })}
          </div>
        </div>
      </div>

      <div className="flex">
        <div className="w-[38%] shrink-0 border-r border-zinc-800">
          <div className="flex items-center gap-1 border-b border-zinc-800 bg-[#1a232e] px-2 py-1.5 font-mono text-xs text-zinc-400">
            <span className="flex-1">Service & Operation</span>
            <span className="text-[10px]">› › » 0µs</span>
          </div>
          {spans.map((span) => (
            <div key={span.span_id} className="flex items-center gap-1 border-b border-zinc-800/50 px-2 py-[3px] hover:bg-zinc-800/40" style={{ paddingLeft: `${8 + span.depth * 14}px` }}>
              <span className="text-[10px] text-zinc-600">{span.depth > 0 ? '›' : '∨'}</span>
              <span className="h-3 w-[3px] shrink-0 rounded-sm" style={{ background: barColor(span.service) }} />
              <span className="truncate font-mono text-xs text-zinc-300">{span.service}</span>
              <span className="ml-1 hidden truncate text-[11px] text-zinc-500 xl:inline">{span.operation}</span>
              <span className="ml-auto shrink-0 font-mono text-[11px] text-zinc-500">{span.duration_ms > 10 ? `${span.duration_ms}ms` : span.duration_ms < 1 ? `${(span.duration_ms * 1000).toFixed(0)}µs` : `${span.duration_ms}ms`}</span>
            </div>
          ))}
        </div>

        <div className="flex-1 overflow-hidden">
          <div className="flex border-b border-zinc-800 bg-[#1a232e] font-mono text-[11px] text-zinc-500">
            {ticks.map((t) => (
              <div key={t} className="flex-1 border-r border-zinc-800/50 px-1 py-1 text-center last:border-r-0">{t === 0 ? '0µs' : `${t}ms`}</div>
            ))}
          </div>
          <div className="relative">
            {ticks.slice(1).map((t) => (
              <div key={t} className="absolute inset-y-0 w-px bg-zinc-800/60" style={{ left: `${(t / totalMs) * 100}%` }} />
            ))}
            {spans.map((span) => {
              const left = (span.start_offset_ms / totalMs) * 100
              const width = Math.max((span.duration_ms / totalMs) * 100, 0.8)
              return (
                <div key={span.span_id} className="relative flex h-[22px] items-center border-b border-zinc-800/30">
                  <div className="absolute h-3 rounded-sm" style={{ left: `${Math.min(left, 100 - width)}%`, width: `${Math.min(width, 100 - left)}%`, background: span.service === 'mythical-requester' ? '#0ea5e9' : '#bae6fd', opacity: span.depth === 0 ? 0.95 : 0.7 }} />
                </div>
              )
            })}
          </div>
        </div>
      </div>
    </div>
  )
}

export { MOCK_SPANS, MOCK_TRACE_ID, MOCK_TOTAL_MS }
export type { SpanRow }
