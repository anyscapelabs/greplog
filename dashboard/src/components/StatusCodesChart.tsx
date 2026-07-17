import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

export default function StatusCodesChart() {
  const colors = useChartTheme()
  const option = {
    tooltip: {
      trigger: 'item',
      formatter: '{b}: {c} ({d}%)'
    },
    legend: {
      bottom: '5%',
      left: 'center',
      icon: 'circle',
      itemWidth: 8,
      itemHeight: 8,
      textStyle: {
        fontSize: 11,
        color: colors.label
      }
    },
    series: [
      {
        type: 'pie',
        radius: ['45%', '75%'],
        center: ['50%', '45%'],
        avoidLabelOverlap: false,
        itemStyle: {
          borderRadius: 4,
        },
        label: {
          show: false,
          position: 'center'
        },
        emphasis: {
          label: {
            show: true,
            fontSize: 16,
            fontWeight: 'bold',
            formatter: '{b}\n{c}'
          }
        },
        labelLine: {
          show: false
        },
        data: [
          { value: 1243, name: '2xx', itemStyle: { color: '#22c55e' } },
          { value: 89, name: '3xx', itemStyle: { color: '#3b82f6' } },
          { value: 342, name: '4xx', itemStyle: { color: '#eab308' } },
          { value: 27, name: '5xx', itemStyle: { color: '#ef4444' } },
        ]
      }
    ]
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} />
}
