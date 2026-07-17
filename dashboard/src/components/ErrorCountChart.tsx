import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

export default function ErrorCountChart() {
  const times = Array.from({ length: 60 }, (_, i) => {
    const min = String(11 + Math.floor(i / 6)).padStart(2, '0')
    const sec = String((i % 6) * 10).padStart(2, '0')
    return `15:${min}:${sec}`
  })

  const data = Array.from({ length: 60 }, () => {
    const r = Math.random()
    if (r > 0.85) return Math.floor(Math.random() * 15 + 5)
    if (r > 0.6) return Math.floor(Math.random() * 5 + 1)
    return 0
  })
  const colors = useChartTheme()

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
    series: [
      {
        type: 'bar',
        data: data,
        itemStyle: { color: '#f87171' },
        barWidth: '70%',
      },
    ],
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} />
}
