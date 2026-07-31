import ReactECharts from 'echarts-for-react'
import ChartEmptyState from './ChartEmptyState.tsx'
import { useChartTheme, commonGrid } from '../utils/useChartTheme.ts'
import type { ServiceHealthEntry } from '../types/index.ts'

interface ServiceHealthChartProps {
  services: ServiceHealthEntry[]
}

export default function ServiceHealthChart({ services }: ServiceHealthChartProps) {
  const colors = useChartTheme()
  if (!services || services.length === 0) return <ChartEmptyState />

  const option = {
    grid: { left: 60, right: 8, top: 8, bottom: 24 },
    xAxis: {
      type: 'category',
      data: services.map((s) => s.name),
      ...commonGrid(colors),
    },
    yAxis: { type: 'value', ...commonGrid(colors) },
    series: [
      {
        name: 'Healthy',
        type: 'bar',
        stack: 'total',
        data: services.map((s) => s.healthy),
        itemStyle: { color: colors.green },
      },
      {
        name: 'Degraded',
        type: 'bar',
        stack: 'total',
        data: services.map((s) => s.degraded),
        itemStyle: { color: colors.orange },
      },
      {
        name: 'Down',
        type: 'bar',
        stack: 'total',
        data: services.map((s) => s.down),
        itemStyle: { color: colors.red },
      },
    ],
    tooltip: { trigger: 'axis' },
    legend: { show: true, bottom: 0, textStyle: { color: colors.label, fontSize: 10 } },
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
}
