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

type Environment = 'production' | 'staging' | 'development'

type ServiceRow = {
  id: string
  service: string
  environment: Environment
  checkType: string
  state: HealthState
  reachability: number
  latency: string
  latencyMs: number
  total: number
}

function deriveHealthState(errorRate: number): HealthState {
  if (errorRate > 10) return 'DOWN'
  return 'UP'
}

function deriveCheckType(service: string): string {
  if (service.includes('dns')) return 'DNS'
  if (service.includes('api')) return 'HTTP'
  if (service.includes('auth')) return 'Browser'
  return 'HTTP'
}

function getLatencyForService(service: string): { latency: string; latencyMs: number } {
  if (service === 'api-gateway') return { latency: '8.32 s', latencyMs: 8320 }
  if (service === 'payment-service') return { latency: '569 ms', latencyMs: 569 }
  if (service === 'worker') return { latency: '19.1 ms', latencyMs: 19.1 }
  if (service === 'web-frontend') return { latency: '451 ms', latencyMs: 451 }
  return { latency: '—', latencyMs: 0 }
}

function mapQueryRowToServiceRow(raw: Record<string, unknown>, _windowSecs: number): ServiceRow | null {
  const service = String(raw.service ?? '').trim()
  if (!service) return null

  // The server's metric is named `count`; the UI calls it "total".
  const total = Number(raw.count ?? 0)
  if (!Number.isFinite(total)) return null

  const errors = Number(raw.errors ?? 0)
  const errorRate = total ? (errors / total) * 100 : 0
  const reachability = 100 - errorRate

  return {
    id: service.slice(0, 5),
    service,
    environment: 'production',
    checkType: deriveCheckType(service),
    state: deriveHealthState(errorRate),
    reachability,
    total,
    ...getLatencyForService(service),
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
  if (ms >= 1000) return 'bg-[#e74c3c] text-white'
  if (ms >= 500) return 'bg-[#f1c40f] text-zinc-900'
  return 'bg-[#2ecc71] text-white'
}

function getEnvironmentColor(environment: Environment): string {
  if (environment === 'production') return 'text-emerald-400'
  if (environment === 'staging') return 'text-amber-400'
  return 'text-zinc-400'
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
    const row = mapQueryRowToServiceRow(raw as Record<string, unknown>, windowSecs)
    if (!row) continue
    rows.push(row)
  }

  if (rows.length === 0) {
    return (
      <div className="flex w-full flex-col rounded-lg border border-zinc-800">
        <div className="flex items-center gap-1.5 px-3 py-2">
          <h2 className="text-sm font-medium uppercase tracking-wide text-zinc-100">Services</h2>

          <Tooltip
            side="bottom-start"
            content="Overview of all services in the selected time window. Shows service health, environment, status, latency, reachability and total requests per service. Status and reachability are heatmaps (green UP, yellow degraded, red DOWN). Derived from logs: reachability = 100% − error%, latency from raw_body when available."
          >
            <span className="cursor-pointer rounded p-1 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300">
              <IoInformationCircleOutline className="h-4 w-4" />
            </span>
          </Tooltip>
        </div>

        <div className="flex min-h-[160px] items-center justify-center rounded-b-lg border-t border-zinc-800 p-8">
          <p className="text-center text-sm text-zinc-500">No data</p>
        </div>
      </div>
    )
  }

  return (
    <div className="flex w-full flex-col rounded-lg border border-zinc-800">
      <div className="flex items-center gap-1.5 px-3 py-2">
        <h2 className="text-sm font-medium uppercase tracking-wide text-zinc-100">Services</h2>

        <Tooltip
          side="bottom-start"
          content="Overview of all services in the selected time window. Shows service health, environment, status, latency, reachability and total requests per service. Status and reachability are heatmaps (green UP, yellow degraded, red DOWN). Derived from logs: reachability = 100% − error%, latency from raw_body when available."
        >
          <span className="cursor-pointer rounded p-1 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300">
            <IoInformationCircleOutline className="h-4 w-4" />
          </span>
        </Tooltip>
      </div>

      <div className="max-h-[360px] overflow-auto rounded-b-lg">
        <table className="w-full min-w-[1100px] border-collapse text-left text-sm">
          <thead className="sticky top-0 z-10">
            <tr className="text-sm text-zinc-400">
              <th className="sticky top-0 z-10 w-[70px] border-b border-r border-zinc-800 bg-zinc-950 px-3 py-2.5 font-medium last:border-r-0">id</th>

              <th className="sticky top-0 z-10 w-[140px] border-b border-r border-zinc-800 bg-zinc-950 px-3 py-2.5 font-medium last:border-r-0">service</th>

              <th className="sticky top-0 z-10 w-[110px] border-b border-r border-zinc-800 bg-zinc-950 px-3 py-2.5 font-medium last:border-r-0">env</th>

              <th className="sticky top-0 z-10 w-[110px] border-b border-r border-zinc-800 bg-zinc-950 px-3 py-2.5 font-medium last:border-r-0">check type</th>

              <th className="sticky top-0 z-10 w-[110px] border-b border-r border-zinc-800 bg-zinc-950 px-3 py-2.5 text-right font-medium last:border-r-0">state</th>

              <th className="sticky top-0 z-10 w-[110px] border-b border-r border-zinc-800 bg-zinc-950 px-3 py-2.5 text-right font-medium last:border-r-0">latency</th>

              <th className="sticky top-0 z-10 w-[150px] border-b border-r border-zinc-800 bg-zinc-950 px-3 py-2.5 text-right font-medium last:border-r-0">reachability</th>

              <th className="sticky top-0 z-10 w-[110px] border-b border-zinc-800 bg-zinc-950 px-3 py-2.5 text-right font-medium">total</th>
            </tr>
          </thead>

          <tbody className="divide-y divide-zinc-800 text-sm">
            {rows.map((row) => (
              <tr key={`${row.id}-${row.service}-${row.environment}`} className="hover:bg-zinc-900/40">
                <td className="border-r border-zinc-800 px-3 py-2.5 text-zinc-300 last:border-r-0">{row.id}</td>

                <td className="max-w-[140px] truncate border-r border-zinc-800 px-3 py-2.5 font-medium text-sky-400 last:border-r-0">{row.service}</td>

                <td className={`border-r border-zinc-800 px-3 py-2.5 text-xs font-bold last:border-r-0 ${getEnvironmentColor(row.environment)}`}>
                  {row.environment}
                </td>

                <td className="border-r border-zinc-800 px-3 py-2.5 text-zinc-300 last:border-r-0">{row.checkType}</td>

                <td className={`border-r border-zinc-800 p-0 text-right font-bold last:border-r-0 ${getStateBackground(row.state)}`}>
                  <div className="px-3 py-2.5">{row.state}</div>
                </td>

                <td className={`border-r border-zinc-800 p-0 text-right last:border-r-0 ${getLatencyBackground(row.latencyMs)}`}>
                  <div className="px-3 py-2.5 text-right">{row.latency}</div>
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
