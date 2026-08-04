import { useState, useEffect, useRef } from 'react'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import { LuClock } from 'react-icons/lu'
import Dropdown from './Dropdown.tsx'

const TIME_RANGES = [
  { label: 'Last 6 hours', value: 'Last 6 hours' },
  { label: 'Last 15 minutes', value: 'Last 15 minutes' },
  { label: 'Last 3 hours', value: 'Last 3 hours' },
  { label: 'Last 7 days', value: 'Last 7 days' },
  { label: 'Last 30 days', value: 'Last 30 days' },
]

export default function TotalEventsCard() {
  const [timeRange, setTimeRange] = useState('Last 6 hours')
  const chartHostRef = useRef<HTMLDivElement>(null)
  const plotRef = useRef<uPlot | null>(null)
  const dataRef = useRef<number[]>([40, 58, 45, 72, 66, 89, 75, 94, 82, 110, 98, 120, 105, 132, 124, 140])

  useEffect(() => {
    const host = chartHostRef.current
    if (!host) return

    const buildPlot = () => {
      const w = host.clientWidth
      const h = host.clientHeight
      if (w <= 0 || h <= 0) return

      plotRef.current?.destroy()
      const ys = dataRef.current
      const xs = ys.map((_, i) => i)

      let yLo = 0
      let yHi = 1
      if (ys.length > 0) {
        const yMin = Math.min(...ys)
        const yMax = Math.max(...ys)
        const range = yMax - yMin || yMax * 0.1 || 1
        yLo = Math.max(0, yMin - range * 0.2)
        yHi = yMax + range * 0.2
      }

      const opts: uPlot.Options = {
        width: w,
        height: h,
        padding: [2, 0, 2, 0],
        legend: { show: false },
        cursor: { show: false },
        select: { show: false },
        scales: {
          x: { time: false, range: () => [0, Math.max(xs.length - 1, 0)] },
          y: { range: () => [yLo, yHi] },
        },
        axes: [{ show: false }, { show: false }],
        series: [
          {},
          {
            stroke: 'rgba(255, 255, 255, 0.9)',
            fill: 'rgba(255, 255, 255, 0.25)',
            width: 1.5,
            points: { show: false },
            paths: uPlot.paths.linear!(),
          },
        ],
      }

      plotRef.current = new uPlot(opts, [xs, ys], host)
    }

    buildPlot()
    const ro = new ResizeObserver(() => buildPlot())
    ro.observe(host)

    return () => {
      ro.disconnect()
      plotRef.current?.destroy()
      plotRef.current = null
    }
  }, [])

  return (
    <div
      className="min-h-40 flex flex-col"
      style={{
        backgroundColor: 'var(--bg-secondary)',
        border: '1px solid var(--border-primary)',
        borderRadius: '10px',
      }}
    >
      <div className="flex items-center justify-between p-2">
        <span className="text-sm font-medium" style={{ color: 'var(--text-primary)' }}>
          Total Events
        </span>
        <Dropdown
          trigger={
            <span className="flex items-center gap-1.5" style={{ color: 'var(--accent)' }}>
              <LuClock className="size-3.5" />
              {timeRange}
            </span>
          }
          items={TIME_RANGES}
          value={timeRange}
          onChange={setTimeRange}
          align="right"
          minWidth="min-w-32"
          triggerClassName="px-2 py-1 text-xs hover:bg-[var(--hover-bg)] rounded"
        />
      </div>
      <div className="border-b" style={{ borderColor: 'var(--border-primary)' }} />
      <div
        className="flex-1 relative"
        style={{
          backgroundColor: 'var(--accent)',
          borderBottomLeftRadius: '10px',
          borderBottomRightRadius: '10px',
          height: '100%',
        }}
      >
        <div
          ref={chartHostRef}
          className="absolute inset-x-0 top-1/2 -translate-y-1/2"
          style={{ height: '50%' }}
        />
      </div>
    </div>
  )
}