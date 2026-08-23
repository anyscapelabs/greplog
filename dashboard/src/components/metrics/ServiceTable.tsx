import { IoInformationCircleOutline } from 'react-icons/io5'
import Tooltip from '../Tooltip'
import { RANGE_SECONDS } from '../logs/Timeline'
import { useServiceTable } from '../../hooks/useLogs'
import type { QueryFilters } from '../../api/logs'
import type { TimeRange } from '../Header'

type Props = {
  range: TimeRange
  filters: QueryFilters
}

type HealthState = 'UP' | 'DOWN'

type ServiceRow = {
  service: string
  state: HealthState
  latency: string
  latencyMs: number
  reachability: number
  total: number
}

function deriveHealthState(errorRate: number): HealthState {
  if (errorRate > 10) return 'DOWN'
  return 'UP'
}

/** Latency lives in the log payload when producers include `latency_ms`. */
function parseLatencyUs(raw: unknown): { latency: string; latencyMs: number } {
  if (typeof raw !== 'string') return { latency: '—', latencyMs: 0 }

  try {
    const payload = JSON.parse(raw) as { latency_ms?: unknown }
    const ms = Number(payload.latency_ms)
    if (!Number.isFinite(ms) || ms < 0) return { latency: '—', latencyMs: 0 }
    return ms >= 1000
      ? { latency: `${(ms / 1000).toFixed(2)} s`, latencyMs: ms }
      : { latency: `${Math.round(ms)} ms`, latencyMs: ms }
  } catch {
    return { latency: '—', latencyMs: 0 }
  }
}

function mapQueryRowToServiceRow(raw: Record<string, unknown>): ServiceRow | null {
  const service = String(raw.service ?? '').trim()
  if (!service) return null

  const total = Number(raw.count ?? 0)
  if (!Number.isFinite(total)) return null

  const errors = Number(raw.errors ?? 0)
  const errorRate = total ? (errors / total) * 100 : 0
  const reachability = 100 - errorRate

  return {
    service,
    state: deriveHealthState(errorRate),
    ...parseLatencyUs(raw.raw_body),
    reachability,
    total,
  }
}

function getStateBackground(state: HealthState): string {
  if (state === 'UP') return 'bg-[#2ecc71] text-white'
  return 'bg-[#e74c3c] text-white'
}

function getReachabilityBackground(value: number): string {
  if (value >= 99.5) return 'bg-[#2ecc71] text-white'
  if (value >= 95) return 'bg-[#f1c40f] text-zinc-900'
  return 'bg-[#e74c3c] text-white'
}

function getLatencyBackground(ms: number): string {
  if (ms === 0) return ''
  if (ms >= 1000) return 'bg-[#e74c3c] text-white'
  if (ms >= 500) return 'bg-[#f1c40f] text-zinc-900'
  return 'bg-[#2ecc71] text-white'
}

export default function ServiceTable({ range, filters }: Props) {
  const { data, isError, error } = useServiceTable(filters)

  if (isError) {
    return (
      <div className="flex w-full flex-col rounded-lg border border-red-800 bg-red-950/20 p-4">
        <p className="text-sm font-medium text-red-400">Failed to load services</p>
        <p className="mt-1 text-xs text-zinc-400">{error instanceof Error ? error.message : 'Unknown error'}</p>
      </div>
    )
  }

  const windowSecs = RANGE_SECONDS[range]

  if (!windowSecs) {
    return (
      <div className="flex w-full flex-col rounded-lg border border-amber-800 bg-amber-950/20 p-4">
        <p className="text-sm text-amber-400">Invalid time range: {String(range)}</p>
      </div>
    )
  }

  const rows: ServiceRow[] = []

  for (const raw of data ?? []) {
    const row = mapQueryRowToServiceRow(raw as Record<string, unknown>)
    if (!row) continue
    rows.push(row)
  }

  const header = (
    <div className="flex items-center gap-1.5 px-3 py-2">
      <h2 className="text-sm font-medium uppercase tracking-wide text-zinc-100">Services</h2>

      <Tooltip
        side="bottom-start"
        content="Overview of all services in the selected time window. Health derives from the error share (UP under 10%), reachability = 100% − error%, and latency comes from `latency_ms` in each log's payload when producers include it."
      >
        <span className="cursor-pointer rounded p-1 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300">
          <IoInformationCircleOutline className="h-4 w-4" />
        </span>
      </Tooltip>
    </div>
  )

  if (rows.length === 0) {
    return (
      <div className="flex w-full flex-col rounded-lg border border-zinc-800">
        {header}

        <div className="flex min-h-[160px] items-center justify-center rounded-b-lg border-t border-zinc-800 p-8">
          <p className="text-center text-sm text-zinc-500">No data</p>
        </div>
      </div>
    )
  }

  return (
    <div className="flex w-full flex-col rounded-lg border border-zinc-800">
      {header}

      <div className="max-h-[360px] overflow-auto rounded-b-lg">
        <table className="w-full min-w-[640px] border-collapse text-left text-sm">
          <thead className="sticky top-0 z-10">
            <tr className="text-sm text-zinc-400">
              <th className="sticky top-0 z-10 border-b border-r border-zinc-800 bg-zinc-950 px-3 py-2.5 font-medium last:border-r-0">service</th>

              <th className="sticky top-0 z-10 w-[110px] border-b border-r border-zinc-800 bg-zinc-950 px-3 py-2.5 text-right font-medium last:border-r-0">state</th>

              <th className="sticky top-0 z-10 w-[110px] border-b border-r border-zinc-800 bg-zinc-950 px-3 py-2.5 text-right font-medium last:border-r-0">latency</th>

              <th className="sticky top-0 z-10 w-[150px] border-b border-r border-zinc-800 bg-zinc-950 px-3 py-2.5 text-right font-medium last:border-r-0">reachability</th>

              <th className="sticky top-0 z-10 w-[110px] border-b border-zinc-800 bg-zinc-950 px-3 py-2.5 text-right font-medium">total</th>
            </tr>
          </thead>

          <tbody className="divide-y divide-zinc-800 text-sm">
            {rows.map((row) => (
              <tr key={row.service} className="hover:bg-zinc-900/40">
                <td className="max-w-[240px] truncate border-r border-zinc-800 px-3 py-2.5 font-medium text-sky-400 last:border-r-0">{row.service}</td>

                <td className={`border-r border-zinc-800 p-0 text-right font-bold last:border-r-0 ${getStateBackground(row.state)}`}>
                  <div className="px-3 py-2.5">{row.state}</div>
                </td>

                <td className={`border-r border-zinc-800 p-0 text-right last:border-r-0 ${getLatencyBackground(row.latencyMs)}`}>
                  <div className="px-3 py-2.5 text-right text-zinc-300">{row.latency}</div>
                </td>

                <td className={`border-r border-zinc-800 p-0 text-right last:border-r-0 ${getReachabilityBackground(row.reachability)}`}>
                  <div className="px-3 py-2.5 text-right">{row.reachability === 100 ? '100%' : `${row.reachability.toFixed(1)}%`}</div>
                </td>

                <td className="px-3 py-2.5 text-right text-zinc-200">{row.total ? row.total.toLocaleString() : '—'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}
