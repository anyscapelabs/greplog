import ReactECharts from 'echarts-for-react'
import ChartEmptyState from './ChartEmptyState.tsx'
import { useChartTheme, commonGrid } from '../utils/useChartTheme.ts'
import type { ErrorRateChartProps } from '../types/index.ts'

export default function ErrorRateChart({ data }: ErrorRateChartProps) {
  const colors = useChartTheme()
  if (!data || data.length === 0) return <ChartEmptyState />

  const { axisLabel: defaultAxisLabel, ...restGrid } = commonGrid(colors)
  const option = {
    grid: { left: 40, right: 8, top: 8, bottom: 24 },
    xAxis: {
      type: 'category',
      data: data.map((d) => {
        const s = d.timestamp
        return s.length > 10 ? s.slice(0, 10) : s
      }),
      ...commonGrid(colors),
    },
    yAxis: {
      type: 'value',
      ...restGrid,
      axisLabel: { ...defaultAxisLabel, formatter: (v: number) => `${(v * 100).toFixed(1)}%` },
    },
    series: [
      {
        type: 'line',
        data: data.map((d) => d.rate),
        smooth: false,
        showSymbol: false,
        lineStyle: { color: colors.orange, width: 1.5 },
        areaStyle: { color: `${colors.orange}30` },
      },
    ],
    tooltip: { trigger: 'axis', valueFormatter: (v: unknown) => `${(Number(v) * 100).toFixed(1)}%` },
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
}
