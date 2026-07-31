import ReactECharts from 'echarts-for-react'
import ChartEmptyState from './ChartEmptyState.tsx'
import { useChartTheme, commonGrid } from '../utils/useChartTheme.ts'

interface LatencyPercentilesChartProps {
  p50: number[]
  p90: number[]
  p99: number[]
  labels: string[]
}

export default function LatencyPercentilesChart({ p50, p90, p99 }: LatencyPercentilesChartProps) {
  const colors = useChartTheme()
  const hasData = p50.length > 0 || p90.length > 0 || p99.length > 0
  if (!hasData) return <ChartEmptyState />

  const valP50 = p50.length > 0 ? p50[0] : 0
  const valP90 = p90.length > 0 ? p90[0] : 0
  const valP99 = p99.length > 0 ? p99[0] : 0

  const option = {
    grid: { left: 50, right: 8, top: 8, bottom: 24 },
    xAxis: {
      type: 'category',
      data: ['p50', 'p90', 'p99'],
      ...commonGrid(colors),
    },
    yAxis: { type: 'value', ...commonGrid(colors) },
    series: [
      {
        type: 'bar',
        data: [
          { value: valP50, itemStyle: { color: colors.blue } },
          { value: valP90, itemStyle: { color: colors.orange } },
          { value: valP99, itemStyle: { color: colors.red } },
        ],
        barWidth: '40%',
      },
    ],
    tooltip: { trigger: 'axis', formatter: (params: any) => `${params[0].name}: ${params[0].value}ms` },
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
}
