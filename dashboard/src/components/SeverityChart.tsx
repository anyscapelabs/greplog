import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

const data = [
  { value: 75000, name: 'INFO', itemStyle: { color: '#3b82f6' } },
  { value: 12000, name: 'WARN', itemStyle: { color: '#f59e0b' } },
  { value: 3000, name: 'ERROR', itemStyle: { color: '#ef4444' } },
  { value: 500, name: 'DEBUG', itemStyle: { color: '#8b5cf6' } }
]

export default function SeverityChart() {
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
