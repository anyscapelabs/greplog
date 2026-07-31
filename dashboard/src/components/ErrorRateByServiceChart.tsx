import ReactECharts from 'echarts-for-react'
import ChartEmptyState from './ChartEmptyState.tsx'
import { useChartTheme, commonGrid } from '../utils/useChartTheme.ts'
import type { ErrorRateByServiceChartProps } from '../types/index.ts'

export default function ErrorRateByServiceChart({ data }: ErrorRateByServiceChartProps) {
  const colors = useChartTheme()
  if (!data || data.length === 0) return <ChartEmptyState />

  const sorted = [...data].sort((a, b) => b.value - a.value)

  const option = {
    grid: { left: 80, right: 8, top: 8, bottom: 24 },
    xAxis: {
      type: 'value',
      axisLabel: { formatter: (v: number) => `${(v * 100).toFixed(0)}%`, ...commonGrid(colors).axisLabel },
      ...commonGrid(colors),
    },
    yAxis: {
      type: 'category',
      data: sorted.map((d) => d.label),
      ...commonGrid(colors),
    },
    series: [
      {
        type: 'bar',
        data: sorted.map((d) => d.value),
        itemStyle: { color: colors.red },
      },
    ],
    tooltip: { trigger: 'axis', valueFormatter: (v: unknown) => `${(Number(v) * 100).toFixed(1)}%` },
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
}
