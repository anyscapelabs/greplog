import { useEffect, useRef } from 'react'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import { useChartTheme } from '../utils/useChartTheme.ts'
import type { LogsHistogramChartProps } from '../types/index.ts'

// Per-minute, per-level stacked bar histogram of log volume, intended to sit at
// the top of the Logs page above the logs table. Data is
// `{ buckets: string[]; levels: { level; counts }[] }`, counts aligned to
// buckets. Levels stack bottom-to-top from debug to critical and use the CSS
// theme color for each level. uPlot in this version has no built-in stacking,
// so values are cumulative and each segment's baseline is drawn via a
// `disp.y0` facet (unit = scale value).
const LEVEL_COLOR: Record<string, keyof ChartColors> = {
  trace: 'green',
  debug: 'green',
  info: 'blue',
  warn: 'orange',
  error: 'red',
  critical: 'red',
  fatal: 'red',
}

// stable stacking order, bottom (first) to top (last)
const LEVEL_RANK: Record<string, number> = {
  trace: 0,
  debug: 1,
  info: 2,
  warn: 3,
  error: 4,
  critical: 5,
  fatal: 6,
}

type ChartColors = ReturnType<typeof useChartTheme>

export default function LogsHistogramChart({ data }: LogsHistogramChartProps) {
  const holderRef = useRef<HTMLDivElement>(null)
  const plotRef = useRef<uPlot | null>(null)
  const colors = useChartTheme()

  useEffect(() => {
    const holder = holderRef.current
    if (!holder || !data || !data.buckets?.length || !data.levels?.length) return

    const labels = data.buckets
    const n = data.buckets.length
    const xs = data.buckets.map((_, i) => i)

    const ordered = [...data.levels].sort(
      (a, b) => (LEVEL_RANK[a.level] ?? 99) - (LEVEL_RANK[b.level] ?? 99),
    )

    // cumulative top-of-stack per level, and the baseline (= top of the level
    // below) used as each segment's y0 facet.
    const acc: number[][] = []
    const offs: number[][] = []
    let run = new Array<number>(n).fill(0)
    for (const lv of ordered) {
      offs.push(run.slice())
      run = run.map((v, i) => v + (lv.counts[i] ?? 0))
      acc.push(run)
    }

    const bars = uPlot.paths!.bars!

    const opts: uPlot.Options = {
      width: holder.clientWidth,
      height: holder.clientHeight,
      legend: { show: false },
      padding: [8, 8, 0, 8],
      scales: {
        x: { time: false },
        y: { distr: 1 },
      },
      axes: [
        {
          stroke: colors.label,
          grid: { stroke: colors.gridStrong, width: 1 },
          ticks: { stroke: 'rgba(0,0,0,0)' },
          font: '10px system-ui',
          values: (_uu, ticks) => ticks.map((i) => labels[i] ?? ''),
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
        ...ordered.map((lv, k) => {
          const base = colors[LEVEL_COLOR[lv.level] ?? 'label']
          const paths = bars({
            disp: {
              y0: {
                unit: 1,
                kind: 2,
                values: (_self, _si, i0, i1) => offs[k].slice(i0, i1),
              },
            },
          })
          return {
            label: lv.level,
            stroke: base,
            fill: base,
            width: 1,
            paths,
            points: { show: false },
          }
        }) as uPlot.Series[],
      ],
    }

    plotRef.current = new uPlot(opts, [xs, ...acc] as uPlot.AlignedData, holder)

    const onResize = () => {
      plotRef.current?.setSize({
        width: holder.clientWidth,
        height: holder.clientHeight,
      })
    }
    const ro = new ResizeObserver(onResize)
    ro.observe(holder)

    return () => {
      ro.disconnect()
      plotRef.current?.destroy()
      plotRef.current = null
    }
  }, [data, colors])

  return <div ref={holderRef} className="w-full h-full" />
}