import { useEffect, useMemo, useRef, useState } from 'react'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import ChartEmptyState from './ChartEmptyState.tsx'
import { useChartTheme } from '../utils/useChartTheme.ts'
import type { LogsHistogramChartProps } from '../types/index.ts'

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
  const hoverRef = useRef<HoverState | null>(null)
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

  const hasData = data.buckets.length > 0 && orderedLevels.some((series) => series.counts.some((count) => count > 0))

  useEffect(() => {
    const host = plotHostRef.current
    if (!host || !hasData) return

    const drawPlot = () => {
      const width = host.clientWidth
      const height = host.clientHeight
      if (width <= 0 || height <= 0) return

      plotRef.current?.destroy()

      const xVals = data.buckets.map((_, index) => index)
      const step = labelStep(data.buckets.length, width)

      const opts: uPlot.Options = {
        width,
        height,
        padding: [8, 12, 0, 8],
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
            auto: false,
            range: (_u, _min, max) => [0, Math.max(1, max * 1.08)],
          },
        },
        axes: [
          {
            stroke: colors.label,
            grid: { stroke: colors.gridStrong, width: 1 },
            ticks: { stroke: 'rgba(0,0,0,0)' },
            font: '10px system-ui',
            values: (_u, ticks) =>
              ticks.map((tick) => {
                const bucketIndex = Number(tick)
                if (!Number.isFinite(bucketIndex)) return ''
                return bucketIndex % step === 0 ? (data.buckets[bucketIndex] ?? '') : ''
              }),
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
          {
            stroke: 'rgba(0,0,0,0)',
            width: 0,
            points: { show: false },
          },
        ],
        hooks: {
          draw: [
            (u) => {
              const ctx = u.ctx
              const centers = xVals.map((value) => u.valToPos(value, 'x', true))
              const span = centers.length > 1 ? Math.max(1, Math.min(...centers.slice(1).map((center, i) => center - centers[i]))) : u.bbox.width
              const barWidth = Math.max(6, Math.floor(span * 0.72))

              ctx.save()

              for (let bucketIndex = 0; bucketIndex < centers.length; bucketIndex += 1) {
                let running = 0
                for (const series of orderedLevels) {
                  const count = series.counts[bucketIndex] ?? 0
                  if (count <= 0) continue

                  const y0 = u.valToPos(running, 'y', true)
                  const y1 = u.valToPos(running + count, 'y', true)
                  const left = Math.round(centers[bucketIndex] - barWidth / 2)
                  const top = Math.round(y1)
                  const rectHeight = Math.max(1, Math.round(y0 - y1))

                  ctx.fillStyle = series.color
                  ctx.fillRect(left, top, barWidth, rectHeight)
                  running += count
                }
              }

              const activeHover = hoverRef.current
              if (activeHover) {
                const bucketIndex = activeHover.index
                if (bucketIndex >= 0 && bucketIndex < centers.length) {
                  ctx.save()
                  ctx.strokeStyle = colors.label
                  ctx.globalAlpha = 0.35
                  ctx.lineWidth = 1
                  ctx.beginPath()
                  ctx.moveTo(centers[bucketIndex], u.bbox.top)
                  ctx.lineTo(centers[bucketIndex], u.bbox.top + u.bbox.height)
                  ctx.stroke()
                  ctx.restore()
                }
              }

              ctx.restore()
            },
          ],
          setCursor: [
            (u) => {
              const left = u.cursor.left
              if (left == null || Number.isNaN(left)) {
                hoverRef.current = null
                setHover(null)
                plotRef.current?.redraw()
                return
              }
              const index = u.posToIdx(left)
              if (index < 0 || index >= data.buckets.length) {
                hoverRef.current = null
                setHover(null)
                plotRef.current?.redraw()
                return
              }
              const tooltipLeft = Math.min(Math.max(12, u.bbox.left + left + 12), Math.max(12, host.clientWidth - 190))
              hoverRef.current = { index, left: tooltipLeft }
              setHover((prev) => {
                if (prev && prev.index === index && prev.left === tooltipLeft) return prev
                return { index, left: tooltipLeft }
              })
              plotRef.current?.redraw()
            },
          ],
        },
      }

      plotRef.current = new uPlot(opts, [xVals, totals], host)
    }

    drawPlot()

    const ro = new ResizeObserver(() => {
      drawPlot()
    })
    ro.observe(host)

    const clearHover = () => {
      hoverRef.current = null
      setHover(null)
      plotRef.current?.redraw()
    }
    host.addEventListener('mouseleave', clearHover)

    return () => {
      host.removeEventListener('mouseleave', clearHover)
      ro.disconnect()
      plotRef.current?.destroy()
      plotRef.current = null
    }
  }, [colors, data.buckets, hasData, orderedLevels, totals])

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
      <div className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-1 px-2 pb-1 pt-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
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
