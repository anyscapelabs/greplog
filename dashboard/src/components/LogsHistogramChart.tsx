import { useEffect, useMemo, useRef, useState } from 'react'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import ChartEmptyState from './ChartEmptyState.tsx'
import { useChartTheme } from '../utils/useChartTheme.ts'
import type { LogsHistogramChartProps } from '../types/index.ts'

// Stacked bar histogram of log volume per minute, per level. Built directly on
// uPlot's `bars` path builder using the `disp.y0`/`disp.y1` facets (both must
// be supplied together — see uPlot's bars.js path builder) rather than a
// custom canvas draw hook: this is the officially supported way to render
// stacked bars in uPlot, and it's what keeps z-order, hit-testing and the
// built-in cursor/legend machinery correct for free. Levels stack
// bottom-to-top from highest to lowest severity (error at the bottom, like
// Grafana's logs histogram), each level's own (non-cumulative) counts are
// kept in `u.data` for tooltip/legend display, and the cumulative baseline
// (y0) / top (y1) arrays are supplied as separate facets.
const LEVEL_META = {
  fatal: { rank: 0, label: 'Fatal', color: 'red' },
  critical: { rank: 1, label: 'Critical', color: 'red' },
  error: { rank: 2, label: 'Error', color: 'red' },
  warning: { rank: 3, label: 'Warning', color: 'orange' },
  warn: { rank: 3, label: 'Warning', color: 'orange' },
  info: { rank: 4, label: 'Info', color: 'green' },
  debug: { rank: 5, label: 'Debug', color: 'blue' },
  trace: { rank: 6, label: 'Trace', color: 'label' },
} as const

type ChartColors = ReturnType<typeof useChartTheme>
type LevelColorKey = keyof ChartColors

type HistogramLevel = {
  level: string
  label: string
  color: string
  counts: number[]
}

type HoverState = {
  index: number
  left: number
}

function fallbackLabel(level: string): string {
  if (!level) return 'Unknown'
  return level.charAt(0).toUpperCase() + level.slice(1)
}

function levelRank(level: string): number {
  return LEVEL_META[level.toLowerCase() as keyof typeof LEVEL_META]?.rank ?? 99
}

function levelLabel(level: string): string {
  return LEVEL_META[level.toLowerCase() as keyof typeof LEVEL_META]?.label ?? fallbackLabel(level)
}

function levelColor(level: string, colors: ChartColors): string {
  const key = LEVEL_META[level.toLowerCase() as keyof typeof LEVEL_META]?.color as LevelColorKey | undefined
  return key ? colors[key] : colors.label
}

function labelStep(bucketCount: number, width: number): number {
  const approxVisible = Math.max(1, Math.floor(width / 56))
  return Math.max(1, Math.ceil(bucketCount / approxVisible))
}

export default function LogsHistogramChart({ data }: LogsHistogramChartProps) {
  const plotHostRef = useRef<HTMLDivElement>(null)
  const plotRef = useRef<uPlot | null>(null)
  const [hover, setHover] = useState<HoverState | null>(null)
  const colors = useChartTheme()

  const orderedLevels = useMemo<HistogramLevel[]>(() => {
    return [...data.levels]
      .sort((a, b) => levelRank(a.level) - levelRank(b.level))
      .map((series) => ({
        level: series.level,
        label: levelLabel(series.level),
        color: levelColor(series.level, colors),
        counts: series.counts,
      }))
  }, [data.levels, colors])

  const totals = useMemo(
    () => data.buckets.map((_, bucketIndex) => orderedLevels.reduce((sum, series) => sum + (series.counts[bucketIndex] ?? 0), 0)),
    [data.buckets, orderedLevels],
  )

  const totalsMax = useMemo(() => totals.reduce((max, value) => Math.max(max, value), 0), [totals])

  // Cumulative baseline (y0) and top (y1) per level, bottom level first, so
  // each series' bar spans [baseline, baseline+ownCount].
  const { baselines, tops } = useMemo(() => {
    const n = data.buckets.length
    let running = new Array<number>(n).fill(0)
    const baselineArrs: number[][] = []
    const topArrs: number[][] = []
    for (const series of orderedLevels) {
      const baseline = running.slice()
      const top = running.map((value, i) => value + (series.counts[i] ?? 0))
      baselineArrs.push(baseline)
      topArrs.push(top)
      running = top
    }
    return { baselines: baselineArrs, tops: topArrs }
  }, [data.buckets.length, orderedLevels])

  const hasData = data.buckets.length > 0 && totalsMax > 0

  useEffect(() => {
    const host = plotHostRef.current
    if (!host || !hasData) return

    const bars = uPlot.paths!.bars!
    const xVals = data.buckets.map((_, index) => index)

    const drawPlot = () => {
      const width = host.clientWidth
      const height = host.clientHeight
      if (width <= 0 || height <= 0) return

      plotRef.current?.destroy()

      const step = labelStep(data.buckets.length, width)
      const yMax = Math.max(1, totalsMax * 1.08)

      const opts: uPlot.Options = {
        width,
        height,
        padding: [10, 8, 0, 8],
        legend: { show: false },
        cursor: {
          x: true,
          y: false,
          lock: false,
          drag: { x: false, y: false },
          points: { show: false },
        },
        scales: {
          x: {
            time: false,
            range: (_u, min, max) => [min - 0.6, max + 0.6],
          },
          y: {
            range: () => [0, yMax],
          },
        },
        axes: [
          {
            stroke: colors.label,
            grid: { stroke: colors.gridStrong, width: 1 },
            ticks: { stroke: 'rgba(0,0,0,0)' },
            font: '10px system-ui',
            splits: (_u, _axisIdx, scaleMin, scaleMax) => {
              const out: number[] = []
              const first = Math.ceil(scaleMin)
              for (let i = first; i <= scaleMax; i += step) out.push(i)
              return out
            },
            values: (_u, ticks) => ticks.map((tick) => data.buckets[tick] ?? ''),
          },
          {
            stroke: colors.label,
            grid: { stroke: colors.grid, width: 1 },
            ticks: { stroke: 'rgba(0,0,0,0)' },
            font: '10px system-ui',
          },
        ],
        series: [
          {},
          ...orderedLevels.map((series, k) => ({
            label: series.label,
            stroke: series.color,
            fill: series.color,
            width: 1,
            points: { show: false },
            paths: bars({
              size: [0.85, 24, 1],
              disp: {
                y0: { unit: 1, values: () => baselines[k] },
                y1: { unit: 1, values: () => tops[k] },
              },
            }),
          })) as uPlot.Series[],
        ],
        hooks: {
          setCursor: [
            (u) => {
              const left = u.cursor.left
              const idx = u.cursor.idx
              if (left == null || left < 0 || idx == null) {
                setHover((prev) => (prev === null ? prev : null))
                return
              }
              const tooltipLeft = Math.min(Math.max(12, left + 12), Math.max(12, width - 190))
              setHover((prev) => {
                if (prev && prev.index === idx && prev.left === tooltipLeft) return prev
                return { index: idx, left: tooltipLeft }
              })
            },
          ],
        },
      }

      plotRef.current = new uPlot(opts, [xVals, ...orderedLevels.map((series) => series.counts)], host)
    }

    drawPlot()

    const ro = new ResizeObserver(() => {
      drawPlot()
    })
    ro.observe(host)

    const clearHover = () => setHover((prev) => (prev === null ? prev : null))
    host.addEventListener('mouseleave', clearHover)

    return () => {
      host.removeEventListener('mouseleave', clearHover)
      ro.disconnect()
      plotRef.current?.destroy()
      plotRef.current = null
    }
  }, [baselines, colors, data.buckets, hasData, orderedLevels, tops, totalsMax])

  if (!hasData) {
    return <ChartEmptyState message="No logs in the selected time range." />
  }

  const hoverBucket = hover ? data.buckets[hover.index] : null
  const hoverValues = hover
    ? orderedLevels
        .map((series) => ({
          label: series.label,
          color: series.color,
          value: series.counts[hover.index] ?? 0,
        }))
        .filter((entry) => entry.value > 0)
    : []

  return (
    <div className="relative flex h-full w-full min-h-0 flex-col overflow-hidden">
      <div className="relative min-h-0 flex-1">
        <div ref={plotHostRef} className="h-full w-full" />
        {hover && hoverBucket && hoverValues.length > 0 && (
          <div
            className="pointer-events-none absolute z-10 min-w-44 rounded border px-3 py-2 text-xs shadow-lg"
            style={{
              left: hover.left,
              top: 10,
              backgroundColor: 'color-mix(in srgb, var(--bg-secondary) 92%, black)',
              borderColor: 'var(--border-primary)',
              color: 'var(--text-primary)',
            }}
          >
            <div className="mb-2 font-medium" style={{ color: 'var(--text-secondary)' }}>{hoverBucket}</div>
            <div className="flex flex-col gap-1">
              {hoverValues.map((entry) => (
                <div key={entry.label} className="flex items-center justify-between gap-4">
                  <div className="flex items-center gap-2">
                    <span className="inline-block h-2.5 w-2.5 rounded-sm" style={{ backgroundColor: entry.color }} />
                    <span>{entry.label}</span>
                  </div>
                  <span className="font-mono">{entry.value}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
      <div className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-0.5 px-2 py-1 text-xs" style={{ color: 'var(--text-secondary)' }}>
        {orderedLevels.map((series) => (
          <div key={series.level} className="flex items-center gap-1.5">
            <span className="inline-block h-2.5 w-2.5 rounded-sm" style={{ backgroundColor: series.color }} />
            <span>{series.label}</span>
          </div>
        ))}
      </div>
    </div>
  )
}
