import ReactECharts from 'echarts-for-react'
import ChartEmptyState from './ChartEmptyState.tsx'
import { useChartTheme } from '../utils/useChartTheme.ts'
import type { ErrorByServiceChartProps } from '../types/index.ts'

export default function ErrorByServiceChart({ data }: ErrorByServiceChartProps) {
  const colors = useChartTheme()
  if (!data || data.length === 0) return <ChartEmptyState />

  const option = {
    grid: { left: 0, right: 0, top: 0, bottom: 0 },
    series: [
      {
        type: 'pie',
        radius: ['30%', '70%'],
        center: ['50%', '50%'],
        data: data.map((d) => ({ name: d.name, value: d.value, itemStyle: { color: d.color } })),
        label: { color: colors.label, fontSize: 10 },
      },
    ],
    tooltip: { trigger: 'item', formatter: '{b}: {c} ({d}%)' },
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
}
