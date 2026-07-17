import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

interface ErrorCountChartProps {
  metric?: string
  groupBy?: string
}

export default function ErrorCountChart({ metric = 'count', groupBy = 'nothing' }: ErrorCountChartProps) {
  const times = Array.from({ length: 60 }, (_, i) => {
    const min = String(11 + Math.floor(i / 6)).padStart(2, '0')
    const sec = String((i % 6) * 10).padStart(2, '0')
    return `15:${min}:${sec}`
  })

  const barColor = metric === 'rate' ? '#fb923c' : '#f87171'

  const data = Array.from({ length: 60 }, () => {
    const r = Math.random()
    if (r > 0.85) return Math.floor(Math.random() * 15 + 5)
    if (r > 0.6) return Math.floor(Math.random() * 5 + 1)
    return 0
  })

  const factor = metric === 'rate' ? 100 : 1

  const colors = useChartTheme()

  let series
  if (groupBy !== 'nothing') {
    const groups = ['web', 'api', 'db']
    const groupedData = groups.map(() =>
      Array.from({ length: 60 }, () => {
        const r = Math.random()
        if (r > 0.85) return Math.floor(Math.random() * 15 + 5)
        if (r > 0.6) return Math.floor(Math.random() * 5 + 1)
        return 0
      })
    )
    const groupColors = ['#f87171', '#fb923c', '#a78bfa']
    series = groups.map((name, i) => ({
      type: 'bar',
      name,
      stack: 'total',
      data: groupedData[i].map(v => v / factor),
      itemStyle: { color: metric === 'rate' ? '#fb923c' : groupColors[i] },
      barWidth: '70%',
    }))
  } else {
    series = [
      {
        type: 'bar',
        data: data.map(v => v / factor),
        itemStyle: { color: barColor },
        barWidth: '70%',
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
