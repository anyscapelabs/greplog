import ReactECharts from 'echarts-for-react'

interface AxisRange {
  min: number
  max: number
}

interface ServiceCardProps {
  name: string
  requests: string
  data: number[]
}

export default function ServiceCard({ name, requests, data }: ServiceCardProps) {
  const accent = getComputedStyle(document.documentElement).getPropertyValue('--accent').trim()
  const option = {
    grid: { left: -1, right: -1, top: 60, bottom: 0 },
    xAxis: { type: 'category', show: false, boundaryGap: false },
    yAxis: {
      type: 'value',
      show: false,
      min: (value: AxisRange) => {
        const range = value.max - value.min || value.max * 0.1 || 1
        return Math.max(0, value.min - range * 0.2)
      },
      max: (value: AxisRange) => {
        const range = value.max - value.min || value.max * 0.1 || 1
        return value.max + range * 0.2
      },
    },
    series: [
      {
        data,
        type: 'line',
        smooth: false,
        showSymbol: false,
        lineStyle: { color: accent, width: 1.5 },
        areaStyle: { color: `${accent}80` },
      },
    ],
    tooltip: { show: false },
  }

  return (
    <div
      className="rounded border h-36 relative overflow-hidden flex flex-col p-3 flex-1 min-w-0"
      style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}
    >
      <div className="z-10 flex flex-col relative">
        <span className="text-sm font-medium" style={{ color: 'var(--text-secondary)' }}>{name}</span>
        <span className="text-xl font-bold mt-1 tracking-tight" style={{ color: 'var(--text-primary)' }}>{requests}</span>
      </div>
      <div className="absolute inset-0 z-0 pointer-events-none">
        <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
      </div>
    </div>
  )
}
