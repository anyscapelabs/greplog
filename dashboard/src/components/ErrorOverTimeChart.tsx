import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

const times = Array.from({ length: 96 }, (_, i) => {
  const h = String(Math.floor(i / 4)).padStart(2, '0')
  const m = String((i % 4) * 15).padStart(2, '0')
  return `${h}:${m}`
})

const errorData = Array.from({ length: 96 }, () => Math.random() * 2)

export default function ErrorOverTimeChart() {
  const colors = useChartTheme()
  const option = {
    tooltip: { trigger: 'axis' },
    grid: { left: '3%', right: '2%', bottom: '16%', top: '8%' },
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
    series: [{
      type: 'line',
      smooth: false,
      symbol: 'none',
      data: errorData,
      itemStyle: { color: '#dc2626' },
      areaStyle: {
        color: {
          type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
          colorStops: [
            { offset: 0, color: 'rgba(220, 38, 38, 0.4)' },
            { offset: 1, color: 'rgba(220, 38, 38, 0.05)' }
          ]
        }
      }
    }]
  }
  return (
    <ReactECharts
      option={option}
      style={{ height: '100%', width: '100%' }}
    />
  )
}
