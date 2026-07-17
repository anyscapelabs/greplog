import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

const times = Array.from({ length: 96 }, (_, i) => {
  const h = String(Math.floor(i / 4)).padStart(2, '0')
  const m = String((i % 4) * 15).padStart(2, '0')
  return `${h}:${m}`
})

const data = Array.from({ length: 96 }, () => Math.floor(Math.random() * 50 + 10))

export default function IngestionChart() {
  const colors = useChartTheme()
  const option = {
    tooltip: { trigger: 'axis' },
    grid: { left: '3%', right: '2%', bottom: '16%', top: '8%' },
    xAxis: {
      type: 'category',
      data: times,
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: { fontSize: 10, color: colors.label, interval: 5 }
    },
    yAxis: {
      type: 'value',
      splitLine: { lineStyle: { color: colors.grid } },
      axisLine: { show: false },
      axisLabel: { fontSize: 10, color: colors.label }
    },
    series: [{ type: 'bar', data, itemStyle: { color: '#3b82f6' }, barWidth: '60%' }]
  }
  return (
    <ReactECharts
      option={option}
      style={{ height: '100%', width: '100%' }}
    />
  )
}
