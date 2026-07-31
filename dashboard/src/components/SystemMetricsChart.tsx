import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'
import type { SystemMetricsChartProps } from '../types/index.ts'

function formatThroughput(bytesPerSec: number): string {
  if (bytesPerSec >= 1_000_000) return `${(bytesPerSec / 1_000_000).toFixed(1)} MB/s`
  if (bytesPerSec >= 1_000) return `${(bytesPerSec / 1_000).toFixed(1)} KB/s`
  return `${bytesPerSec} B/s`
}

function GaugeTile({ title, value, format, color }: { title: string; value: number | null; format: (v: number) => string; color: string }) {
  const colors = useChartTheme()
  if (value === null) {
    return (
      <div className="flex flex-col items-center justify-center gap-1 h-full">
        <span className="text-xs" style={{ color: 'var(--text-secondary)' }}>{title}</span>
        <span className="text-sm" style={{ color: 'var(--text-secondary)' }}>No data</span>
      </div>
    )
  }
  const isPercent = format(value).endsWith('%')
  const max = isPercent ? 100 : Math.max(value * 1.3, 1)
  const option = {
    series: [
      {
        type: 'gauge',
        min: 0,
        max,
        startAngle: 220,
        endAngle: -40,
        progress: { show: true, width: 8, itemStyle: { color } },
        axisLine: { lineStyle: { width: 8, color: [[1, colors.grid]] } },
        pointer: { show: false },
        axisTick: { show: false },
        splitLine: { show: false },
        axisLabel: { show: false },
        title: { offsetCenter: [0, '42%'], fontSize: 10, color: colors.label },
        detail: {
          valueAnimation: true,
          formatter: format(value),
          color: colors.label,
          fontSize: 14,
          offsetCenter: [0, '0%'],
        },
        data: [{ value }],
      },
    ],
  }
  return (
    <div className="flex flex-col items-center h-full">
      <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
      <span className="text-xs -mt-4" style={{ color: 'var(--text-secondary)' }}>{title}</span>
    </div>
  )
}

export default function SystemMetricsChart({ cpu, memory, diskIO, network }: SystemMetricsChartProps) {
  const colors = useChartTheme()
  const last = (arr: number[]): number | null => (arr.length > 0 ? arr[arr.length - 1] : null)

  return (
    <div className="grid grid-cols-4 h-full w-full gap-1 p-1">
      <GaugeTile title="CPU" value={last(cpu)} format={(v) => `${v.toFixed(0)}%`} color={colors.blue} />
      <GaugeTile title="Memory" value={last(memory)} format={(v) => `${v.toFixed(0)}%`} color={colors.green} />
      <GaugeTile title="Disk" value={last(diskIO)} format={(v) => `${v.toFixed(0)}%`} color={colors.orange} />
      <GaugeTile title="Network" value={last(network)} format={formatThroughput} color={colors.red} />
    </div>
  )
}
