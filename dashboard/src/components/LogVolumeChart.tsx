import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

interface LogVolumeChartProps {
  metric?: string
  groupBy?: string
}

export default function LogVolumeChart({ metric = 'count', groupBy = 'nothing' }: LogVolumeChartProps) {
  const times = Array.from({ length: 60 }, (_, i) => {
    const min = String(11 + Math.floor(i / 6)).padStart(2, '0')
    const sec = String((i % 6) * 10).padStart(2, '0')
    return `15:${min}:${sec}`
  })

  // Generate random data for Total Requests
  const data = Array.from({ length: 60 }, () => Math.floor(Math.random() * 400 + 200))

  const isRate = metric === 'rate'
  const mainColor = isRate ? '#3b82f6' : '#4ade80'
  const mainColorStart = isRate ? 'rgba(59, 130, 246, 0.4)' : 'rgba(74, 222, 128, 0.4)'
  const mainColorEnd = isRate ? 'rgba(59, 130, 246, 0.05)' : 'rgba(74, 222, 128, 0.05)'

  const adjustedData = isRate ? data.map(v => v * 0.1) : data

  const colors = useChartTheme()

  const series: any[] = [
    {
      type: 'line',
      smooth: false,
      symbol: 'none',
      data: adjustedData,
      itemStyle: { color: mainColor },
      lineStyle: { width: 2, color: mainColor },
      areaStyle: {
        color: {
          type: 'linear',
          x: 0,
          y: 0,
          x2: 0,
          y2: 1,
          colorStops: [
            { offset: 0, color: mainColorStart },
            { offset: 1, color: mainColorEnd },
          ],
        },
      },
    },
  ]

  if (groupBy !== 'nothing') {
    const secondData = Array.from({ length: 60 }, () => Math.floor(Math.random() * 300 + 100))
    const adjustedSecond = isRate ? secondData.map(v => v * 0.1) : secondData
    series.push({
      type: 'line',
      smooth: false,
      symbol: 'none',
      data: adjustedSecond,
      itemStyle: { color: '#a78bfa' },
      lineStyle: { width: 2, color: '#a78bfa' },
      areaStyle: {
        color: {
          type: 'linear',
          x: 0,
          y: 0,
          x2: 0,
          y2: 1,
          colorStops: [
            { offset: 0, color: 'rgba(167, 139, 250, 0.4)' },
            { offset: 1, color: 'rgba(167, 139, 250, 0.05)' },
          ],
        },
      },
    })
  }

  const option = {
    tooltip: { trigger: 'axis' },
    grid: { left: '8%', right: '5%', bottom: '15%', top: '10%' },
    xAxis: {
      type: 'category',
      data: times,
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: {
        fontSize: 10,
        color: colors.label,
        hideOverlap: true,
      },
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
