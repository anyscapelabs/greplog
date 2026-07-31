import ReactECharts from 'echarts-for-react'
import ChartEmptyState from './ChartEmptyState.tsx'
import { useChartTheme } from '../utils/useChartTheme.ts'
import type { PieSlice } from '../types/index.ts'

interface SeverityChartProps {
  data: PieSlice[]
}

export default function SeverityChart({ data }: SeverityChartProps) {
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
        emphasis: {
          itemStyle: { shadowBlur: 10, shadowColor: 'rgba(0,0,0,0.3)' },
        },
      },
    ],
    tooltip: { trigger: 'item', formatter: '{b}: {c} ({d}%)' },
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
}
