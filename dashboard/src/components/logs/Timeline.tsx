import { useEffect, useMemo, useRef, useState } from 'react'
import {
  LuArrowDownToLine,
  LuArrowUpToLine,
  LuChevronLeft,
  LuChevronRight,
} from 'react-icons/lu'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import type { TimeRange } from '../Header'
import type { QueryRow } from '../../api/logs'

const MIN_TIMELINE_HEIGHT = 160
const DEFAULT_TIMELINE_HEIGHT = 200
const MAX_TIMELINE_HEIGHT = 420
const TIMELINE_CHART_OFFSET = 32

export const RANGE_SECONDS: Record<TimeRange, number> = {
  '15m': 900,
  '1h': 3600,
  '3h': 10800,
  '6h': 21600,
  '12h': 43200,
  '24h': 86400,
  '7d': 604800,
  '30d': 2592000,
}

const HISTOGRAM_CONFIG: Partial<
  Record<TimeRange, { spacing: number; bars: number; labels: number }>
> = {
  '15m': { spacing: 15, bars: 60, labels: 60 },
  '1h': { spacing: 60, bars: 60, labels: 300 },
  '3h': { spacing: 300, bars: 36, labels: 600 },
  '6h': { spacing: 600, bars: 36, labels: 1800 },
  '12h': { spacing: 1200, bars: 36, labels: 1800 },
  '24h': { spacing: 1800, bars: 48, labels: 3600 },
  '7d': { spacing: 14400, bars: 42, labels: 43200 },
  '30d': { spacing: 43200, bars: 60, labels: 172800 },
}

/** Bucket width in seconds for a given time range.
 *
 * The histogram is binned at this interval so every chart renders its
 * configured number of bars (`RANGE_SECONDS[range] / spacing`), e.g. 60 bars
 * for the last 15 minutes and 60 bars for the last hour.
 */
export function binIntervalSeconds(range: TimeRange): number {
  return HISTOGRAM_CONFIG[range]?.spacing ?? 60
}

function formatAxisTime(timestamp: number, spacing: number): string {
  const date = new Date(timestamp * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  if (spacing < 60) {
    return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  }
  if (spacing <= 3600) {
    return `${pad(date.getHours())}:${pad(date.getMinutes())}`
  }
  return `${pad(date.getDate())}/${pad(date.getMonth() + 1)}`
}

function parseBucketSeconds(bucket: unknown): number | null {
  if (typeof bucket === 'number') {
    return bucket >= 1e12 ? Math.floor(bucket / 1e6) : Math.floor(bucket)
  }
  if (typeof bucket !== 'string') return null
  const iso = /Z$|[+-]\d{2}:\d{2}$/.test(bucket) ? bucket : `${bucket}Z`
  const ms = Date.parse(iso)
  if (Number.isNaN(ms)) return null
  return Math.floor(ms / 1000)
}

const DEFAULT_BAR_COLOR = { fill: 'rgba(138, 180, 248, 0.5)', stroke: '#8ab4f8' }

const SEVERITY_BAR_COLORS: Record<
  string,
  { fill: string; stroke: string }
> = {
  DEBUG: { fill: 'rgba(161, 161, 170, 0.6)', stroke: '#a1a1aa' },
  INFO: { fill: 'rgba(56, 189, 248, 0.6)', stroke: '#38bdf8' },
  WARN: { fill: 'rgba(251, 191, 36, 0.6)', stroke: '#fbbf24' },
  ERROR: { fill: 'rgba(248, 113, 113, 0.6)', stroke: '#f87171' },
}

interface DragState {
  start: number
  base: number
}

interface TimelineProps {
  fullscreen: boolean
  range: TimeRange
  shift: number
  histogram?: QueryRow[]
  /** Active severity facet (e.g. "ERROR"); colors the bars to match. */
  severity?: string
  onShiftChange: (shift: number) => void
  /** Section title shown above the chart, e.g. "Timeline" or "Ingestion". */
  title?: string
}

function Timeline({
  fullscreen,
  range,
  shift,
  histogram,
  severity,
  onShiftChange,
  title = 'Timeline',
}: TimelineProps) {
  const [collapsed, setCollapsed] = useState(false)
  const [height, setHeight] = useState(DEFAULT_TIMELINE_HEIGHT)
  const [hover, setHover] = useState<{
    idx: number
    x: number
    y: number
  } | null>(null)
  const dragRef = useRef<DragState | null>(null)
  const chartFrameRef = useRef<HTMLDivElement>(null)
  const chartTargetRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<uPlot | null>(null)
  const config = HISTOGRAM_CONFIG[range]
  const bars = config?.bars ?? 60
  const spacing = config?.spacing ?? RANGE_SECONDS[range] / bars
  const barColors = useMemo(
    () => (severity ? SEVERITY_BAR_COLORS[severity] ?? DEFAULT_BAR_COLOR : DEFAULT_BAR_COLOR),
    [severity],
  )
  const histogramData: [number[], number[]] = useMemo(() => {
    const end = Math.floor(Date.now() / 1000) - shift
    const start = end - RANGE_SECONDS[range]
    // date_bin aligns buckets to the Unix epoch, so every boundary must be an
    // epoch-aligned multiple of `spacing`.
    const first = Math.ceil(start / spacing) * spacing
    const last = Math.floor(end / spacing) * spacing
    const byBucket = new Map<number, number>()
    for (const row of histogram ?? []) {
      const bucket = parseBucketSeconds(row.bucket)
      if (bucket == null) continue
      byBucket.set(bucket, Number(row.count) || 0)
    }
    const times: number[] = []
    const counts: number[] = []
    for (let t = first; t <= last; t += spacing) {
      times.push(t)
      counts.push(byBucket.get(t) ?? 0)
    }
    return [times, counts]
  }, [histogram, range, spacing, shift])

  const labelInterval = HISTOGRAM_CONFIG[range]?.labels ?? 0

  const rangeEnd = Math.floor(Date.now() / 1000) - shift
  const rangeStart = rangeEnd - RANGE_SECONDS[range]

  const ticks: number[] = useMemo(() => {
    if (!labelInterval) return []
    const first = Math.ceil(rangeStart / labelInterval) * labelInterval
    const result: number[] = []
    for (let t = first; t <= rangeEnd; t += labelInterval) {
      result.push(t)
    }
    return result
  }, [labelInterval, rangeStart, rangeEnd])

  useEffect(() => {
    const frame = chartFrameRef.current
    if (!frame) return

    const opts: uPlot.Options = {
      width: frame.clientWidth,
      height: frame.clientHeight,
      padding: [0, 0, 0, 0],
      cursor: { show: true, x: false, y: false, focus: { prox: 10 } },
      legend: { show: false },
      axes: [
        { show: false },
        { show: false },
      ],
      series: [
        {},
        {
          label: 'Logs',
          fill: barColors.fill,
          stroke: barColors.stroke,
          points: { show: false },
          paths: uPlot.paths.bars!({ size: [0.9, 100] }),
        },
      ],
      hooks: {
        setCursor: [
          (u: uPlot) => {
            const { idx } = u.cursor
            const left = u.cursor.left ?? 0
            const top = u.cursor.top ?? 0
            const target = chartFrameRef.current
            if (idx == null || !target) {
              setHover(null)
              return
            }
            const w = target.offsetWidth
            const tooltipW = 120
            const tooltipH = 42
            const x = Math.min(Math.max(left + 14, 4), w - tooltipW - 4)
            const y = top > tooltipH + 12 ? top - tooltipH - 12 : top + 14
            setHover({ idx, x, y })
          },
        ],
      },
    }

    chartRef.current = new uPlot(opts, histogramData, chartTargetRef.current!)

    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect
        if (chartRef.current) {
          chartRef.current.setSize({ width, height })
        }
      }
    })
    resizeObserver.observe(frame)

    return () => {
      resizeObserver.disconnect()
      if (chartRef.current) {
        chartRef.current.destroy()
        chartRef.current = null
      }
    }
  }, [histogramData, barColors])

  useEffect(() => {
    const frame = chartFrameRef.current
    if (!frame || !chartRef.current) return
    chartRef.current.setSize({
      width: frame.clientWidth,
      height: frame.clientHeight,
    })
  }, [height])

  const stopDrag = () => {
    dragRef.current = null
    window.removeEventListener('pointermove', onPointerMove)
    window.removeEventListener('pointerup', stopDrag)
  }

  const onPointerMove = (event: PointerEvent) => {
    const state = dragRef.current
    if (!state) return
    const next = state.base + (event.clientY - state.start)
    const clamped = Math.min(
      MAX_TIMELINE_HEIGHT,
      Math.max(MIN_TIMELINE_HEIGHT, next),
    )
    setHeight(clamped)
    const frame = chartFrameRef.current
    if (chartRef.current && frame) {
      chartRef.current.setSize({
        width: frame.clientWidth,
        height: clamped - TIMELINE_CHART_OFFSET,
      })
    }
  }

  const startDrag = (event: React.PointerEvent) => {
    event.preventDefault()
    dragRef.current = {
      start: event.clientY,
      base: height,
    }
    window.addEventListener('pointermove', onPointerMove)
    window.addEventListener('pointerup', stopDrag)
  }

  return (
    <section className="shrink-0 border-b border-zinc-800">
      <div className="flex items-center justify-between px-3 py-2">
        <h2 className="text-xs font-medium uppercase tracking-wide text-zinc-100">
          {title}
        </h2>
        <button
          type="button"
          onClick={() => setCollapsed((value) => !value)}
          className="cursor-pointer text-zinc-500 transition-colors hover:text-zinc-300"
        >
          {collapsed ? (
            <LuArrowDownToLine className="h-4 w-4" />
          ) : (
            <LuArrowUpToLine className="h-4 w-4" />
          )}
        </button>
      </div>
      {!collapsed && !fullscreen && (
        <div style={{ height }} className="relative flex flex-col">
          <div className="flex flex-1">
            <div className="flex w-10 shrink-0 flex-col items-center justify-center">
              <button
                type="button"
                onClick={() => onShiftChange(shift + spacing)}
                className="cursor-pointer rounded-md p-1 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300"
              >
                <LuChevronLeft className="h-4 w-4" />
              </button>
            </div>
            <div
              ref={chartFrameRef}
              className="relative flex-1 overflow-hidden border-b border-l-2 border-r-2 border-zinc-700 [border-left-style:dotted] [border-right-style:dotted]"
            >
              <div ref={chartTargetRef} className="h-full w-full overflow-hidden" />
              {hover && (
                <div
                  className="pointer-events-none absolute z-10 flex flex-col gap-0.5 rounded-md border border-zinc-700 bg-zinc-800 px-2 py-1 font-mono text-[10px] text-zinc-100 shadow-lg"
                  style={{ left: hover.x, top: hover.y }}
                >
                  <span className="text-zinc-400">
                    {formatAxisTime(histogramData[0][hover.idx], spacing)}
                  </span>
                  <span className="text-white">
                    {histogramData[1][hover.idx]} logs
                  </span>
                </div>
              )}
            </div>
            <div className="flex w-10 shrink-0 flex-col items-center justify-center">
              <button
                type="button"
                onClick={() => onShiftChange(shift - spacing)}
                className="cursor-pointer rounded-md p-1 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300"
              >
                <LuChevronRight className="h-4 w-4" />
              </button>
            </div>
          </div>
          <div className="mb-2 flex h-6 shrink-0">
            <div className="w-10 shrink-0" />
            <div className="relative flex-1 overflow-hidden bg-zinc-900">
              {ticks.map((timestamp) => {
                const fraction =
                  (timestamp - rangeStart) / (rangeEnd - rangeStart)
                return (
                  <span
                    key={timestamp}
                    className="absolute top-1/2 -translate-x-1/2 -translate-y-1/2 font-mono text-[10px] leading-none text-zinc-500"
                    style={{ left: `${fraction * 100}%` }}
                  >
                    {formatAxisTime(timestamp, labelInterval)}
                  </span>
                )
              })}
            </div>
            <div className="w-10 shrink-0" />
          </div>
          <div
            onPointerDown={startDrag}
            className="absolute inset-x-0 bottom-0 h-1.5 cursor-row-resize bg-transparent transition-colors hover:bg-zinc-700"
          />
        </div>
      )}
    </section>
  )
}

export default Timeline
