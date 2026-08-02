import ReactECharts from 'echarts-for-react'
import ChartEmptyState from './ChartEmptyState.tsx'
import { useChartTheme, commonGrid } from '../utils/useChartTheme.ts'
import type { LogsHistogramChartProps } from '../types/index.ts'

// A per-time-bucket bar histogram of log volume, intended to sit at the top
// of the Logs page above the logs table. Data is `{ timestamp, count }[]`
// already bucketed (ascending by timestamp) by the useLogs hook.
export default function LogsHistogramChart({ data }: LogsHistogramChartProps) {
  const colors = useChartTheme()
  if (!data || data.length === 0) return <ChartEmptyState />

  const option = {
    grid: { left: 40, right: 8, top: 8, bottom: 24 },
    xAxis: {
      type: 'category' as const,
      data: data.map((d) => d.timestamp),
      ...commonGrid(colors),
    },
    yAxis: {
      type: 'value' as const,
      minInterval: 1,
      ...commonGrid(colors),
    },
    series: [
      {
        type: 'bar' as const,
        data: data.map((d) => d.count),
        barWidth: '60%',
        itemStyle: { color: colors.blue, opacity: 0.85 },
      },
    ],
    tooltip: { trigger: 'axis' },
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
}