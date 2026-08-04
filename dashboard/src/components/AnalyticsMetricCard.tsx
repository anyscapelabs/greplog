import { useEffect, useRef } from 'react'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import { useChartTheme } from '../utils/useChartTheme.ts'

interface AnalyticsMetricCardProps {
  title: string
  value: string
  color: string
  rgb: string
  data: number[]
}

// Grafana Stat-panel style metric card.
// Layout: colored left accent bar | title (small) / value (large bold) | uPlot sparkline flushed to right half
// The sparkline is a minimal uPlot line+area chart with no axes, no grid, no tooltip — purely decorative.
// rgb prop drives the sparkline fill color (semi-transparent).
export default function AnalyticsMetricCard({ title, value, color, data }: AnalyticsMetricCardProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const plotHostRef = useRef<HTMLDivElement>(null)
  const plotRef = useRef<uPlot | null>(null)
  const colors = useChartTheme()

  useEffect(() => {
    const host = plotHostRef.current
    if (!host) return

    const hasSparkline = data.length > 1
    if (!hasSparkline) return

    const buildPlot = () => {
      const w = host.clientWidth
      const h = host.clientHeight
      if (w <= 0 || h <= 0) return

      plotRef.current?.destroy()

      const xs = data.map((_, i) => i)
      const ys = data

      // Compute y range with 20% breathing room so the sparkline doesn't
      // clip at the edges — same approach used by ECharts previously.
      const yMin = Math.min(...ys)
      const yMax = Math.max(...ys)
      const range = yMax - yMin || yMax * 0.1 || 1
      const yLo = Math.max(0, yMin - range * 0.2)
      const yHi = yMax + range * 0.2

      // Hex → rgba for fill
      function hexToRgba(hex: string, alpha: number): string {
        const clean = hex.replace('#', '')
        if (clean.length !== 6) return hex
        const r = parseInt(clean.slice(0, 2), 16)
        const g = parseInt(clean.slice(2, 4), 16)
        const b = parseInt(clean.slice(4, 6), 16)
        if ([r, g, b].some((c) => Number.isNaN(c))) return hex
        return `rgba(${r}, ${g}, ${b}, ${alpha})`
      }

      const opts: uPlot.Options = {
        width: w,
        height: h,
        padding: [2, 0, 2, 0],
        legend: { show: false },
        cursor: { show: false },
        select: { show: false },
        scales: {
          x: { time: false, range: () => [0, xs.length - 1] },
          y: { range: () => [yLo, yHi] },
        },
        axes: [{ show: false }, { show: false }],
        series: [
          {},
          {
            stroke: color,
            fill: hexToRgba(color, 0.18),
            width: 1.5,
            points: { show: false },
            paths: uPlot.paths.linear!(),
          },
        ],
      }

      plotRef.current = new uPlot(opts, [xs, ys], host)
    }

    buildPlot()

    const ro = new ResizeObserver(() => { buildPlot() })
    ro.observe(host)

    return () => {
      ro.disconnect()
      plotRef.current?.destroy()
      plotRef.current = null
    }
  }, [data, color, colors])

  const hasSparkline = data.length > 1

  return (
    <div
      ref={containerRef}
      className="relative h-24 flex overflow-hidden"
      style={{
        backgroundColor: 'var(--bg-secondary)',
        border: '1px solid var(--border-primary)',
        borderRadius: '2px',
      }}
    >
      {/* Grafana-style left accent bar */}
      <div
        className="absolute left-0 top-0 bottom-0 w-0.5"
        style={{ backgroundColor: color }}
      />

      {/* Text content — z-indexed over sparkline */}
      <div className="relative z-10 flex flex-col justify-center pl-3 pr-2 min-w-0" style={{ flex: '0 0 auto', maxWidth: '60%' }}>
        <span
          className="text-xs font-medium uppercase tracking-wide truncate"
          style={{ color: 'var(--text-secondary)', letterSpacing: '0.06em' }}
        >
          {title}
        </span>
        <span
          className="text-2xl font-bold mt-0.5 leading-tight tabular-nums"
          style={{ color }}
        >
          {value}
        </span>
      </div>

      {/* Sparkline — right half, absolutely positioned, pointer-events none */}
      {hasSparkline && (
        <div
          ref={plotHostRef}
          className="absolute right-0 top-0 bottom-0 pointer-events-none"
          style={{ width: '55%', overflow: 'hidden' }}
        />
      )}
    </div>
  )
}
