import ReactECharts from 'echarts-for-react'
import ChartEmptyState from './ChartEmptyState.tsx'
import { useChartTheme, commonGrid } from '../utils/useChartTheme.ts'
import type { TimeseriesPoint } from '../types/index.ts'

interface IngestionChartProps {
  data: TimeseriesPoint[]
}

export default function IngestionChart({ data }: IngestionChartProps) {
  const colors = useChartTheme()
  if (!data || data.length === 0) return <ChartEmptyState />

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
    yAxis: { type: 'value', ...commonGrid(colors) },
    series: [
      {
        type: 'line',
        data: data.map((d) => d.value),
        smooth: false,
        showSymbol: false,
        lineStyle: { color: colors.blue, width: 1.5 },
        areaStyle: { color: `${colors.blue}30` },
      },
    ],
    tooltip: { trigger: 'axis' },
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
}
