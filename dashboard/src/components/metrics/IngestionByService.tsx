import { useEffect, useMemo, useRef } from 'react'
import { IoInformationCircleOutline } from 'react-icons/io5'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import Tooltip from '../Tooltip'
import { binIntervalSeconds, RANGE_SECONDS } from '../logs/Timeline'
import { useIngestionByService } from '../../hooks/useLogs'
import type { QueryFilters } from '../../api/logs'
import type { TimeRange } from '../Header'

type Props = {
  range: TimeRange
  filters?: QueryFilters
  shift?: number
}

const SERVICE_PALETTE = [
  '#38bdf8',
  '#fbbf24',
  '#f87171',
  '#4ade80',
  '#a78bfa',
  '#fb7185',
  '#34d399',
  '#f472b6',
]

function parseBucketSeconds(bucket: unknown): number | null {
  if (typeof bucket === 'number') {
    if (bucket >= 1e12) return Math.floor(bucket / 1e6)
    return Math.floor(bucket)
  }

  if (typeof bucket !== 'string') return null

  const iso = /Z$|[+-]\d{2}:\d{2}$/.test(bucket) ? bucket : `${bucket}Z`
  const ms = Date.parse(iso)
  if (Number.isNaN(ms)) return null

  return Math.floor(ms / 1000)
}

function formatTickLabel(timestampSeconds: number, binSeconds: number): string {
  const date = new Date(timestampSeconds * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')

  if (binSeconds < 60) return `${pad(date.getHours())}:${pad(date.getMinutes())}`
  if (binSeconds <= 3600) return `${pad(date.getHours())}:${pad(date.getMinutes())}`

  return `${pad(date.getDate())}/${pad(date.getMonth() + 1)}`
}

function styleLegendInsideCard(container: HTMLElement): void {
  const legend = container.querySelector('.u-legend') as HTMLElement | null
  if (!legend) return

  legend.style.display = 'flex'
  legend.style.flexWrap = 'wrap'
  legend.style.justifyContent = 'center'
  legend.style.gap = '12px'
  legend.style.paddingTop = '6px'
  legend.style.fontSize = '11px'
  legend.style.maxWidth = '100%'
}

export default function IngestionByService({ range, filters, shift = 0 }: Props) {
  const queryFilters = useMemo<QueryFilters>(() => {
    if (filters) return filters
    return { timeRangeSecs: RANGE_SECONDS[range], facets: {} }
  }, [filters, range])

  const { data, isLoading, isError, error } = useIngestionByService(queryFilters, range)

  const chartFrameRef = useRef<HTMLDivElement>(null)
  const chartTargetRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<uPlot | null>(null)

  const binSeconds = binIntervalSeconds(range)

  const { uplotData, services } = useMemo(() => {
    const nowSeconds = Math.floor(Date.now() / 1000) - shift
    const rangeSeconds = RANGE_SECONDS[range]
    if (!rangeSeconds) return { uplotData: [[], []] as unknown as [number[], ...number[][]], services: [] as string[] }

    const firstBucket = Math.ceil((nowSeconds - rangeSeconds) / binSeconds) * binSeconds
    const lastBucket = Math.floor(nowSeconds / binSeconds) * binSeconds

    const discovered = new Set<string>()
    for (const row of data ?? []) {
      const service = String((row as Record<string, unknown>).service ?? '').trim()
      if (!service) continue
      discovered.add(service)
    }

    const serviceNames = Array.from(discovered).sort().slice(0, 8)
    if (serviceNames.length === 0) {
      // No data — return empty shape; caller shows "no data"
      const emptyTimes: number[] = []
      for (let t = firstBucket; t <= lastBucket; t += binSeconds) emptyTimes.push(t)
      return { uplotData: [emptyTimes] as unknown as [number[], ...number[][]], services: [] as string[] }
    }

    const countsByBucket = new Map<number, Map<string, number>>()

    for (const row of data ?? []) {
      const bucket = parseBucketSeconds((row as Record<string, unknown>).bucket)
      if (bucket === null) continue

      const service = String((row as Record<string, unknown>).service ?? '').trim()
      if (!serviceNames.includes(service)) continue

      const count = Number((row as Record<string, unknown>).count) || 0

      if (!countsByBucket.has(bucket)) countsByBucket.set(bucket, new Map())
      countsByBucket.get(bucket)!.set(service, count)
    }

    const times: number[] = []
    const seriesArrays: number[][] = serviceNames.map(() => [])

    for (let t = firstBucket; t <= lastBucket; t += binSeconds) {
      times.push(t)
      const bucketCounts = countsByBucket.get(t)
      serviceNames.forEach((svc, idx) => {
        seriesArrays[idx].push(bucketCounts?.get(svc) ?? 0)
      })
    }

    return { uplotData: [times, ...seriesArrays] as unknown as [number[], ...number[][]], services: serviceNames }
  }, [data, range, binSeconds, shift])

  useEffect(() => {
    const frame = chartFrameRef.current
    const target = chartTargetRef.current
    if (!frame || !target) return
    if (services.length === 0) return

    const legendHeight = 28
    const plotHeight = Math.max(160, frame.clientHeight - legendHeight)

    const series: uPlot.Series[] = [
      {},
      ...services.map((serviceName, index) => ({
        label: serviceName,
        stroke: SERVICE_PALETTE[index % SERVICE_PALETTE.length],
        width: 1.5,
        points: { show: false },
        spanGaps: true,
      })),
    ]

    const options: uPlot.Options = {
      width: frame.clientWidth,
      height: plotHeight,
      padding: [8, 8, 0, 8],
      cursor: { show: true, x: true, y: true },
      legend: { show: true, live: true },
      scales: { x: { time: true } },
      axes: [
        {
          stroke: '#a1a1aa',
          grid: { show: true, stroke: 'rgba(63,63,70,0.35)', width: 1 },
          ticks: { show: true, stroke: 'rgba(63,63,70,0.5)', width: 1, size: 4 },
          font: '10px monospace',
          gap: 4,
          space: 70,
          values: (_u, splits) => splits.map((v) => formatTickLabel(v, binSeconds)),
        },
        {
          stroke: '#a1a1aa',
          grid: { show: true, stroke: 'rgba(63,63,70,0.2)', width: 1 },
          ticks: { show: true, stroke: 'rgba(63,63,70,0.5)', width: 1, size: 4 },
          font: '10px monospace',
          gap: 4,
          size: 40,
        },
      ],
      series,
    }

    chartRef.current = new uPlot(options, uplotData as unknown as uPlot.AlignedData, target)
    styleLegendInsideCard(target)

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (!chartRef.current) continue
        const { width, height } = entry.contentRect
        chartRef.current.setSize({ width, height: Math.max(160, height - legendHeight) })
      }
    })

    observer.observe(frame)

    return () => {
      observer.disconnect()
      if (chartRef.current) {
        chartRef.current.destroy()
        chartRef.current = null
      }
    }
  }, [uplotData, services, binSeconds])

  if (isError) {
    return (
      <div className="flex h-[350px] min-h-[350px] w-full flex-col rounded-lg border border-red-800 bg-red-950/20 p-4">
        <p className="text-sm font-medium text-red-400">Failed to load ingestion by service</p>
        <p className="mt-1 text-xs text-zinc-400">{error instanceof Error ? error.message : 'Unknown error'}</p>
      </div>
    )
  }

  return (
    <div className="flex h-[350px] min-h-[350px] w-full flex-col rounded-lg border border-zinc-800">
      <div className="flex items-center gap-1.5 px-3 py-2">
        <h2 className="text-xs font-medium uppercase tracking-wide text-zinc-100">Ingestion by Service</h2>

        <Tooltip
          side="bottom-start"
          content="Per-service log volume per bucket over the selected time range — helps spot noisy services (Hive-partitioned service=*)."
        >
          <span className="cursor-pointer rounded p-1 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300">
            <IoInformationCircleOutline className="h-4 w-4" />
          </span>
        </Tooltip>

        {isLoading && <span className="ml-2 text-[10px] text-zinc-500">loading…</span>}

        {!isLoading && services.length === 0 && <span className="ml-2 text-[10px] text-zinc-500">no data</span>}
      </div>

      <div ref={chartFrameRef} className="min-h-0 flex-1 overflow-hidden px-2 pb-2">
        <div ref={chartTargetRef} className="h-full w-full overflow-hidden" />
      </div>
    </div>
  )
}
