import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

const data = [
  { value: 4200, name: '2xx', itemStyle: { color: '#10b981' } },
  { value: 300, name: '3xx', itemStyle: { color: '#3b82f6' } },
  { value: 150, name: '4xx', itemStyle: { color: '#f59e0b' } },
  { value: 20, name: '5xx', itemStyle: { color: '#ef4444' } }
]

export default function StatusCodesPieChart() {
  const colors = useChartTheme()
  const option = {
    tooltip: { trigger: 'item' },
    legend: {
      bottom: 0, icon: 'circle', itemWidth: 8, itemHeight: 8,
      textStyle: { fontSize: 10, color: colors.label }
    },
    series: [{
      type: 'pie',
      radius: ['40%', '70%'],
      center: ['50%', '45%'],
      avoidLabelOverlap: false,
      itemStyle: { borderRadius: 4, borderColor: '#fff', borderWidth: 2 },
      label: { show: false, position: 'center' },
      emphasis: { label: { show: true, fontSize: '16', fontWeight: 'bold' } },
      labelLine: { show: false },
      data
    }]
  }
  return (
    <ReactECharts
      option={option}
      style={{ height: '100%', width: '100%' }}
    />
  )
}
