import { useEffect, useMemo, useRef } from 'react'
import { IoInformationCircleOutline } from 'react-icons/io5'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import Tooltip from '../Tooltip'
import { binIntervalSeconds, RANGE_SECONDS } from '../logs/Timeline'
import { useSeverityBreakdown } from '../../hooks/useLogs'
import type { QueryFilters } from '../../api/logs'
import type { TimeRange } from '../Header'

type Props = {
  range: TimeRange
  filters?: QueryFilters
  shift?: number
}

type Level = 'DEBUG' | 'INFO' | 'WARN' | 'ERROR'

const LEVEL_COLOR: Record<Level, string> = {
  DEBUG: '#a1a1aa',
  INFO: '#38bdf8',
  WARN: '#fbbf24',
  ERROR: '#f87171',
}

function isLevel(value: string): value is Level {
  return value === 'DEBUG' || value === 'INFO' || value === 'WARN' || value === 'ERROR'
}

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

function buildSeries(label: Level): uPlot.Series {
  return {
    label,
    stroke: LEVEL_COLOR[label],
    width: 1.5,
    points: { show: false },
    spanGaps: true,
  }
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

export default function SeverityBreakdown({ range, filters, shift = 0 }: Props) {
  const queryFilters = useMemo<QueryFilters>(() => {
    if (filters) return filters
    return { timeRangeSecs: RANGE_SECONDS[range], facets: {} }
  }, [filters, range])

  const { data, isLoading, isError, error } = useSeverityBreakdown(queryFilters, range)

  const chartFrameRef = useRef<HTMLDivElement>(null)
  const chartTargetRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<uPlot | null>(null)

  const binSeconds = binIntervalSeconds(range)

  const chartData: [number[], number[], number[], number[], number[]] = useMemo(() => {
    const nowSeconds = Math.floor(Date.now() / 1000) - shift
    const rangeSeconds = RANGE_SECONDS[range]

    if (!rangeSeconds) return [[], [], [], [], []]

    const start = nowSeconds - rangeSeconds
    const firstBucket = Math.ceil(start / binSeconds) * binSeconds
    const lastBucket = Math.floor(nowSeconds / binSeconds) * binSeconds

    const countsByBucket = new Map<number, Map<Level, number>>()

    for (const row of data ?? []) {
      const bucket = parseBucketSeconds((row as Record<string, unknown>).bucket)
      if (bucket === null) continue

      const rawLevel = String((row as Record<string, unknown>).level ?? '').toUpperCase()
      if (!isLevel(rawLevel)) continue

      const count = Number((row as Record<string, unknown>).count) || 0

      if (!countsByBucket.has(bucket)) countsByBucket.set(bucket, new Map())
      countsByBucket.get(bucket)!.set(rawLevel, count)
    }

    const times: number[] = []
    const debug: number[] = []
    const info: number[] = []
    const warn: number[] = []
    const errors: number[] = []

    for (let t = firstBucket; t <= lastBucket; t += binSeconds) {
      const bucketCounts = countsByBucket.get(t)
      times.push(t)
      debug.push(bucketCounts?.get('DEBUG') ?? 0)
      info.push(bucketCounts?.get('INFO') ?? 0)
      warn.push(bucketCounts?.get('WARN') ?? 0)
      errors.push(bucketCounts?.get('ERROR') ?? 0)
    }

    return [times, debug, info, warn, errors]
  }, [data, range, binSeconds, shift])

  const hasData = chartData.slice(1).some((series) => series.some((value) => value > 0))

  useEffect(() => {
    const frame = chartFrameRef.current
    const target = chartTargetRef.current
    if (!frame || !target) return

    const legendHeight = 28
    const plotHeight = Math.max(120, frame.clientHeight - legendHeight)

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
      series: [{}, buildSeries('DEBUG'), buildSeries('INFO'), buildSeries('WARN'), buildSeries('ERROR')],
    }

    chartRef.current = new uPlot(options, chartData as unknown as uPlot.AlignedData, target)
    styleLegendInsideCard(target)

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (!chartRef.current) continue
        const { width, height } = entry.contentRect
        chartRef.current.setSize({ width, height: Math.max(120, height - legendHeight) })
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
  }, [chartData, binSeconds])

  if (isError) {
    return (
      <div className="flex h-[350px] min-h-[350px] w-[68%] flex-col rounded-lg border border-red-800 bg-red-950/20 p-4">
        <p className="text-sm font-medium text-red-400">Failed to load severity</p>
        <p className="mt-1 text-xs text-zinc-400">{error instanceof Error ? error.message : 'Unknown error'}</p>
      </div>
    )
  }

  return (
    <div className="flex h-[350px] min-h-[350px] w-[68%] flex-col rounded-lg border border-zinc-800">
      <div className="flex items-center gap-1.5 px-3 py-2">
        <h2 className="text-xs font-medium uppercase tracking-wide text-zinc-100">Severity Breakdown</h2>

        <Tooltip
          side="bottom-start"
          content="Distribution of log levels (DEBUG, INFO, WARN, ERROR) over the selected time range. Each line shows count per bucket."
        >
          <span className="cursor-pointer rounded p-1 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300">
            <IoInformationCircleOutline className="h-4 w-4" />
          </span>
        </Tooltip>

        {isLoading && <span className="ml-2 text-[10px] text-zinc-500">loading…</span>}
      </div>

      <div ref={chartFrameRef} className="relative min-h-0 flex-1 overflow-hidden px-2 pb-2">
        <div ref={chartTargetRef} className="h-full w-full overflow-hidden" />

        {!isLoading && !hasData && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
            <span className="text-sm text-zinc-500">No data</span>
          </div>
        )}
      </div>
    </div>
  )
}
