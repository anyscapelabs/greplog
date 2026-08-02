import ReactECharts from 'echarts-for-react'

interface AxisRange {
  min: number
  max: number
}

interface AnalyticsMetricCardProps {
  title: string
  value: string
  color: string
  rgb: string
  data: number[]
}

export default function AnalyticsMetricCard({ title, value, color, rgb, data }: AnalyticsMetricCardProps) {
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
        lineStyle: { color, width: 1.5 },
        areaStyle: { color: `rgba(${rgb}, 0.5)` },
      },
    ],
    tooltip: { show: false },
  }

  return (
    <div
      className="rounded border h-36 relative overflow-hidden flex flex-col p-3"
      style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}
    >
      <div className="z-10 flex flex-col relative">
        <span className="text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>{title}</span>
        <span className="text-3xl font-bold mt-1 tracking-tight" style={{ color: 'var(--text-primary)' }}>{value}</span>
      </div>
      <div className="absolute inset-0 z-0 pointer-events-none">
        <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
      </div>
    </div>
  )
}
