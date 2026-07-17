import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

const times = Array.from({ length: 96 }, (_, i) => {
  const h = String(Math.floor(i / 4)).padStart(2, '0')
  const m = String((i % 4) * 15).padStart(2, '0')
  return `${h}:${m}`
})

const latencyP50 = Array.from({ length: 96 }, () => 20 + Math.random() * 10)
const latencyP90 = Array.from({ length: 96 }, () => 50 + Math.random() * 20)
const latencyP99 = Array.from({ length: 96 }, () => 100 + Math.random() * 40)

export default function LatencyPercentilesChart() {
  const colors = useChartTheme()
  const option = {
    tooltip: { trigger: 'axis' },
    legend: {
      data: ['P50', 'P90', 'P99'],
      bottom: 0,
      icon: 'circle',
      itemWidth: 8,
      itemHeight: 8,
      textStyle: { fontSize: 10, color: colors.label }
    },
    grid: { left: '3%', right: '2%', bottom: '24%', top: '8%' },
    xAxis: {
      type: 'category',
      data: times,
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: { fontSize: 10, color: colors.label, interval: 7 }
    },
    yAxis: {
      type: 'value',
      splitLine: { lineStyle: { color: colors.grid } },
      axisLine: { show: false },
      axisLabel: { fontSize: 10, color: colors.label }
    },
    series: [
      { name: 'P50', type: 'line', smooth: false, symbol: 'none', data: latencyP50, itemStyle: { color: '#10b981' } },
      { name: 'P90', type: 'line', smooth: false, symbol: 'none', data: latencyP90, itemStyle: { color: '#f59e0b' } },
      { name: 'P99', type: 'line', smooth: false, symbol: 'none', data: latencyP99, itemStyle: { color: '#ef4444' } }
    ]
  }
  return (
    <ReactECharts
      option={option}
      style={{ height: '100%', width: '100%' }}
    />
  )
}
