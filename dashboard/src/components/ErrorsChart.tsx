import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

interface ErrorsChartProps {
  metric?: string
  groupBy?: string
}

export default function ErrorsChart({ metric = 'count', groupBy = 'nothing' }: ErrorsChartProps) {
  const times = Array.from({ length: 60 }, (_, i) => {
    const min = String(11 + Math.floor(i / 6)).padStart(2, '0')
    const sec = String((i % 6) * 10).padStart(2, '0')
    return `15:${min}:${sec}`
  })

  // Generate random data with mostly zeros and some spikes for errors
  const data = Array.from({ length: 60 }, () => {
    const r = Math.random()
    if (r > 0.8) return Math.floor(Math.random() * 10 + 2)
    if (r > 0.6) return Math.floor(Math.random() * 5 + 1)
    return 0
  })

  const isRate = metric === 'rate'
  const barColor = isRate ? '#fb923c' : '#f87171'
  const adjustedData = isRate ? data.map(v => v / 10) : data

  const colors = useChartTheme()

  const series: any[] = [
    {
      type: 'bar',
      data: adjustedData,
      itemStyle: { color: barColor },
      barWidth: '70%',
    },
  ]

  if (groupBy !== 'nothing') {
    const secondData = Array.from({ length: 60 }, () => {
      const r = Math.random()
      if (r > 0.8) return Math.floor(Math.random() * 8 + 1)
      if (r > 0.6) return Math.floor(Math.random() * 4 + 1)
      return 0
    })
    const adjustedSecond = isRate ? secondData.map(v => v / 10) : secondData
    series.push({
      type: 'bar',
      data: adjustedSecond,
      itemStyle: { color: '#818cf8' },
      barWidth: '70%',
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
