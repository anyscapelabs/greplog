import { formatDuration } from '../../utils/format'

type SpanRow = {
  span_id: string
  parent_span_id: string | null
  trace_id: string
  service: string
  message: string
  duration_ms: number | null
  timestamp_us: number
}

const MOCK_TRACE_ID = 'trace_abc123'

const MOCK_SPANS: SpanRow[] = [
  {
    trace_id: MOCK_TRACE_ID,
    span_id: 'span_root',
    parent_span_id: null,
    service: 'api-gateway',
    message: 'POST /checkout',
    duration_ms: 420,
    timestamp_us: 1723000000000000,
  },
  {
    trace_id: MOCK_TRACE_ID,
    span_id: 'span_auth',
    parent_span_id: 'span_root',
    service: 'auth-service',
    message: 'verify jwt',
    duration_ms: 45,
    timestamp_us: 1723000000010000,
  },
  {
    trace_id: MOCK_TRACE_ID,
    span_id: 'span_inventory',
    parent_span_id: 'span_root',
    service: 'inventory-service',
    message: 'reserve stock',
    duration_ms: 120,
    timestamp_us: 1723000000020000,
  },
  {
    trace_id: MOCK_TRACE_ID,
    span_id: 'span_payment',
    parent_span_id: 'span_root',
    service: 'payment-service',
    message: 'charge card',
    duration_ms: 280,
    timestamp_us: 1723000000030000,
  },
  {
    trace_id: MOCK_TRACE_ID,
    span_id: 'span_payment_db',
    parent_span_id: 'span_payment',
    service: 'payment-service',
    message: 'INSERT payment',
    duration_ms: 95,
    timestamp_us: 1723000000040000,
  },
  {
    trace_id: MOCK_TRACE_ID,
    span_id: 'span_notify',
    parent_span_id: 'span_root',
    service: 'notification-service',
    message: 'send email',
    duration_ms: 60,
    timestamp_us: 1723000000150000,
  },
]

function depthOf(spanId: string, parentId: string | null, spanMap: Map<string, SpanRow>, memo: Map<string, number>): number {
  if (memo.has(spanId)) return memo.get(spanId)!
  if (!parentId || !spanMap.has(parentId)) {
    memo.set(spanId, 0)
    return 0
  }
  const parent = spanMap.get(parentId)!
  const d = 1 + depthOf(parent.span_id, parent.parent_span_id, spanMap, memo)
  memo.set(spanId, d)
  return d
}

export default function TraceWaterfall({ spans = MOCK_SPANS }: { spans?: SpanRow[] }) {
  if (spans.length === 0) return null
  const spanMap = new Map(spans.map((s) => [s.span_id, s]))
  const memo = new Map<string, number>()
  const maxDuration = Math.max(...spans.map((s) => s.duration_ms ?? 0), 1)

  return (
    <div className="p-3 font-mono text-xs">
      <div className="mb-2 text-[11px] text-zinc-500">showing spans within the current time range — trace may be truncated · mock data</div>
      <div className="space-y-1">
        {spans.map((span) => {
          const depth = depthOf(span.span_id, span.parent_span_id, spanMap, memo)
          const width = span.duration_ms != null ? Math.max(4, (span.duration_ms / maxDuration) * 100) : 0
          return (
            <div
              key={span.span_id}
              className="flex items-center gap-2 rounded bg-zinc-900/50 px-2 py-1.5"
              style={{ marginLeft: depth * 16 }}
            >
              <span className="w-24 shrink-0 truncate text-zinc-500">{span.span_id}</span>
              <span className="shrink-0 rounded bg-zinc-800 px-1.5 py-0.5 text-zinc-300">{span.service}</span>
              <span className="min-w-0 flex-1 truncate text-zinc-200">{span.message}</span>
              {span.duration_ms != null && (
                <div className="flex w-28 shrink-0 items-center gap-1.5">
                  <div className="h-1.5 flex-1 rounded bg-zinc-800">
                    <div className="h-1.5 rounded bg-[#a06bff]" style={{ width: `${width}%` }} />
                  </div>
                  <span className="w-12 text-right text-[11px] text-zinc-400">{formatDuration(span.duration_ms)}</span>
                </div>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}

export { MOCK_SPANS, MOCK_TRACE_ID }
