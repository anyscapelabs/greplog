import ReactECharts from 'echarts-for-react'
import ChartEmptyState from './ChartEmptyState.tsx'
import { useChartTheme, commonGrid } from '../utils/useChartTheme.ts'
import type { BarDataPoint } from '../types/index.ts'

interface AvgResponseTimeChartProps {
  data: BarDataPoint[]
}

export default function AvgResponseTimeChart({ data }: AvgResponseTimeChartProps) {
  const colors = useChartTheme()
  if (!data || data.length === 0) {
    return <ChartEmptyState message="No HTTP metrics — request data depends on SDK capture. Upgrade to a recent SDK for response-time coverage." />
  }

  const sorted = [...data].sort((a, b) => b.value - a.value)

  const option = {
    grid: { left: 80, right: 8, top: 8, bottom: 24 },
    xAxis: {
      type: 'value',
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
        itemStyle: { color: colors.orange },
      },
    ],
    tooltip: { trigger: 'axis', formatter: '{b}: {c}ms' },
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
}
