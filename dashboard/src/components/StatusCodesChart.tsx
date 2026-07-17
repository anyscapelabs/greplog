import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

interface StatusCodesChartProps {
  metric?: string
  groupBy?: string
}

export default function StatusCodesChart({ metric = 'count', groupBy = 'nothing' }: StatusCodesChartProps) {
  const colors = useChartTheme()

  const isRate = metric === 'rate'
  const factor = isRate ? 0.1 : 1

  const series: any[] = [
    {
      type: 'pie',
      radius: groupBy !== 'nothing' ? ['30%', '50%'] : ['45%', '75%'],
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
        { value: Math.round(1243 * factor), name: '2xx', itemStyle: { color: '#22c55e' } },
        { value: Math.round(89 * factor), name: '3xx', itemStyle: { color: '#3b82f6' } },
        { value: Math.round(342 * factor), name: '4xx', itemStyle: { color: '#eab308' } },
        { value: Math.round(27 * factor), name: '5xx', itemStyle: { color: '#ef4444' } },
      ]
    }
  ]

  if (groupBy !== 'nothing') {
    series.push({
      type: 'pie',
      radius: ['60%', '80%'],
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
        { value: Math.round(876 * factor), name: 'path-a', itemStyle: { color: '#86efac' } },
        { value: Math.round(456 * factor), name: 'path-b', itemStyle: { color: '#93c5fd' } },
        { value: Math.round(234 * factor), name: 'path-c', itemStyle: { color: '#fde68a' } },
        { value: Math.round(135 * factor), name: 'path-d', itemStyle: { color: '#fca5a5' } },
      ]
    })
  }

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
    series,
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} />
}
