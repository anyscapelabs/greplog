type SpanRow = {
  span_id: string
  parent_span_id: string | null
  trace_id: string
  service: string
  message: string
  duration_ms: number | null
  start_offset_ms: number
}

const MOCK_TRACE_ID = 'trc_8f2a1c'
const MOCK_TOTAL_MS = 812

const MOCK_SPANS: SpanRow[] = [
  { trace_id: MOCK_TRACE_ID, span_id: 'api-gateway', parent_span_id: null, service: 'api-gateway', message: 'POST /api/v1/checkout', duration_ms: 812, start_offset_ms: 0 },
  { trace_id: MOCK_TRACE_ID, span_id: 'auth', parent_span_id: 'api-gateway', service: 'auth-service', message: 'auth', duration_ms: 48, start_offset_ms: 2 },
  { trace_id: MOCK_TRACE_ID, span_id: 'order', parent_span_id: 'api-gateway', service: 'order-service', message: 'order', duration_ms: 162, start_offset_ms: 58 },
  { trace_id: MOCK_TRACE_ID, span_id: 'payment', parent_span_id: 'api-gateway', service: 'payment-service', message: 'payment', duration_ms: 651, start_offset_ms: 112 },
  { trace_id: MOCK_TRACE_ID, span_id: 'stripe', parent_span_id: 'payment', service: 'stripe.charge()', message: 'stripe.charge()', duration_ms: 610, start_offset_ms: 148 },
  { trace_id: MOCK_TRACE_ID, span_id: 'inventory', parent_span_id: 'api-gateway', service: 'inventory-service', message: 'inventory', duration_ms: 31, start_offset_ms: 781 },
]

const SERVICE_COLOR: Record<string, string> = {
  'api-gateway': '#a78bfa',
  gateway: '#a78bfa',
  'auth-service': '#3b82f6',
  auth: '#3b82f6',
  'order-service': '#f59e0b',
  order: '#f59e0b',
  'payment-service': '#ef4444',
  payment: '#ef4444',
  'stripe.charge()': '#fca5a5',
  stripe: '#fca5a5',
  'inventory-service': '#22c55e',
  inventory: '#22c55e',
}

function barColor(service: string, highlight?: boolean): string {
  if (highlight) return '#ef4444'
  return SERVICE_COLOR[service] ?? '#a78bfa'
}

export default function TraceWaterfall({ spans = MOCK_SPANS, traceId = MOCK_TRACE_ID, totalMs = MOCK_TOTAL_MS }: { spans?: SpanRow[], traceId?: string, totalMs?: number }) {
  const distinctServices = new Set(spans.map((s) => s.service)).size
  const timelineMax = Math.max(totalMs, 800)

  return (
    <div className="bg-[#0a0a0a] px-4 py-3 font-mono text-sm">
      <div className="mb-3 flex items-center justify-between text-sm">
        <span className="text-zinc-400">
          Trace <span className="font-semibold text-violet-400">{traceId}</span>
          <span className="text-zinc-500"> · {spans.length} spans across {distinctServices} services</span>
        </span>
        <span className="text-zinc-500">Total <span className="font-semibold text-violet-400">{totalMs}ms</span></span>
      </div>

      <div className="relative mb-2 ml-[160px] h-3 text-xs text-zinc-600">
        {[0, 200, 400, 600, 800].map((t) => (
          <span key={t} className="absolute -translate-x-1/2" style={{ left: `${(t / timelineMax) * 100}%` }}>{t}ms</span>
        ))}
      </div>

      <div className="space-y-2">
        {spans.map((span) => {
          const rawLeft = (span.start_offset_ms / timelineMax) * 100
          const rawWidth = ((span.duration_ms ?? 0) / timelineMax) * 100
          const left = Math.min(rawLeft, 100)
          const width = Math.min(Math.max(rawWidth, 6), 100 - left)
          const color = barColor(span.service, span.service === 'payment-service')
          const isStripe = span.service === 'stripe.charge()'
          return (
            <div key={span.span_id} className="flex items-center gap-3">
              <div className="flex w-[150px] shrink-0 items-center gap-2">
                <span className="h-2 w-2 shrink-0 rounded-full" style={{ background: color }} />
                <span className="truncate text-sm text-zinc-300">{span.service}</span>
              </div>
              <div className="relative h-6 flex-1 overflow-hidden rounded bg-zinc-900">
                <div
                  className="absolute top-1 flex h-4 items-center rounded px-1.5 text-xs font-medium leading-none"
                  style={{ left: `${left}%`, width: `${width}%`, background: color, color: isStripe ? '#111' : '#fff' }}
                >
                  <span className="truncate">{span.duration_ms}ms{isStripe ? ' - timeout' : ''}</span>
                </div>
              </div>
            </div>
          )
        })}
      </div>

      <div className="mt-3 flex flex-wrap gap-3 border-t border-zinc-800 pt-3 text-xs">
        <span className="flex items-center gap-1.5 text-zinc-500"><span className="h-2.5 w-2.5 rounded-sm" style={{ background: '#a78bfa' }} />gateway</span>
        <span className="flex items-center gap-1.5 text-zinc-500"><span className="h-2.5 w-2.5 rounded-sm" style={{ background: '#f59e0b' }} />order</span>
        <span className="flex items-center gap-1.5 text-zinc-500"><span className="h-2.5 w-2.5 rounded-sm bg-red-500" />payment (bottleneck)</span>
        <span className="flex items-center gap-1.5 text-zinc-500"><span className="h-2.5 w-2.5 rounded-sm bg-green-500" />inventory</span>
        <span className="flex items-center gap-1.5 text-zinc-500"><span className="h-2.5 w-2.5 rounded-sm bg-blue-500" />auth</span>
      </div>

      <div className="mt-2 text-xs text-zinc-600">showing spans within the current time range — trace may be truncated · mock data</div>
    </div>
  )
}

export { MOCK_SPANS, MOCK_TRACE_ID, MOCK_TOTAL_MS }
export type { SpanRow }
