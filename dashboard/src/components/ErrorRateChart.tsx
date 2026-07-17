import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

interface ErrorRateChartProps {
  metric?: string
  groupBy?: string
}

export default function ErrorRateChart({ metric = 'count', groupBy = 'nothing' }: ErrorRateChartProps) {
  const times = Array.from({ length: 60 }, (_, i) => {
    const min = String(11 + Math.floor(i / 6)).padStart(2, '0')
    const sec = String((i % 6) * 10).padStart(2, '0')
    return `15:${min}:${sec}`
  })

  const factor = metric === 'rate' ? 2 : 1
  const lineColor = metric === 'rate' ? '#3b82f6' : '#f87171'

  const colors = useChartTheme()

  let series
  if (groupBy !== 'nothing') {
    const groups = ['web', 'api', 'db']
    const groupColors = ['#3b82f6', '#f87171', '#a78bfa']
    series = groups.map((name, i) => {
      const data = Array.from({ length: 60 }, () => (Math.random() * 8 + 0.5) * factor)
      return {
        type: 'line',
        name,
        smooth: true,
        symbol: 'none',
        data,
        lineStyle: { width: 2, color: metric === 'rate' ? '#3b82f6' : groupColors[i] },
        itemStyle: { color: metric === 'rate' ? '#3b82f6' : groupColors[i] },
        areaStyle: {
          color: {
            type: 'linear',
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: metric === 'rate' ? 'rgba(59, 130, 246, 0.3)' : 'rgba(248, 113, 113, 0.3)' },
              { offset: 1, color: metric === 'rate' ? 'rgba(59, 130, 246, 0.05)' : 'rgba(248, 113, 113, 0.05)' },
            ],
          },
        },
      }
    })
  } else {
    const data = Array.from({ length: 60 }, () => (Math.random() * 8 + 0.5) * factor)
    series = [
      {
        type: 'line',
        smooth: true,
        symbol: 'none',
        data,
        lineStyle: { width: 2, color: lineColor },
        itemStyle: { color: lineColor },
        areaStyle: {
          color: {
            type: 'linear',
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: metric === 'rate' ? 'rgba(59, 130, 246, 0.3)' : 'rgba(248, 113, 113, 0.3)' },
              { offset: 1, color: metric === 'rate' ? 'rgba(59, 130, 246, 0.05)' : 'rgba(248, 113, 113, 0.05)' },
            ],
          },
        },
      },
    ]
  }

  const option = {
    tooltip: { trigger: 'axis' },
    grid: { left: '8%', right: '5%', bottom: '15%', top: '10%' },
    xAxis: {
      type: 'category',
      data: times,
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: { fontSize: 10, color: colors.label, hideOverlap: true },
    },
    yAxis: {
      type: 'value',
      splitLine: { lineStyle: { color: colors.gridStrong, width: 1 } },
      axisLine: { show: false },
      axisLabel: { fontSize: 10, color: colors.label },
    },
    series,
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} />
}
