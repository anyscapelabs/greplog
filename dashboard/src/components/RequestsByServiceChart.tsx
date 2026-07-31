import ReactECharts from 'echarts-for-react'
import ChartEmptyState from './ChartEmptyState.tsx'
import { useChartTheme, commonGrid } from '../utils/useChartTheme.ts'
import type { RequestsByServiceChartProps } from '../types/index.ts'

export default function RequestsByServiceChart({ data }: RequestsByServiceChartProps) {
  const colors = useChartTheme()
  if (!data || data.length === 0) return <ChartEmptyState />

  const sorted = [...data].sort((a, b) => b.value - a.value)

  const option = {
    grid: { left: 80, right: 8, top: 8, bottom: 24 },
    xAxis: { type: 'value', ...commonGrid(colors) },
    yAxis: {
      type: 'category',
      data: sorted.map((d) => d.label),
      ...commonGrid(colors),
    },
    series: [
      {
        type: 'bar',
        data: sorted.map((d) => d.value),
        itemStyle: { color: colors.blue },
      },
    ],
    tooltip: { trigger: 'axis' },
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
}
