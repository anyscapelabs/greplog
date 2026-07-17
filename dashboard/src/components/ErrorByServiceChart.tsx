import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

interface ErrorByServiceChartProps {
  metric?: string
  groupBy?: string
}

export default function ErrorByServiceChart({ metric = 'count', groupBy = 'nothing' }: ErrorByServiceChartProps) {
  const colors = useChartTheme()

  const pieData = [
    { value: 234, name: 'web', itemStyle: { color: '#fb923c' } },
    { value: 567, name: 'api', itemStyle: { color: '#f87171' } },
    { value: 89, name: 'db', itemStyle: { color: '#fbbf24' } },
    { value: 123, name: 'worker', itemStyle: { color: '#a78bfa' } },
  ]

  const rateData = pieData.map(d => ({ ...d, value: +(d.value / 100).toFixed(2) }))
  const displayData = metric === 'rate' ? rateData : pieData

  let series
  if (groupBy !== 'nothing') {
    const regions = ['us-east', 'us-west', 'eu-west']
    const regionColors = ['#60a5fa', '#f97316', '#34d399']
    series = regions.map((region, i) => ({
      type: 'pie',
      radius: ['45%', '75%'],
      center: [(25 + i * 25) + '%', '45%'],
      avoidLabelOverlap: false,
      itemStyle: { borderRadius: 4 },
      label: { show: false, position: 'center' },
      emphasis: {
        label: {
          show: true,
          fontSize: 16,
          fontWeight: 'bold',
          formatter: '{b}\n{c} errors',
        },
      },
      labelLine: { show: false },
      name: region,
      data: displayData.map(d => ({ ...d, itemStyle: { color: regionColors[i] } })),
    }))
  } else {
    series = [
      {
        type: 'pie',
        radius: ['45%', '75%'],
        center: ['50%', '45%'],
        avoidLabelOverlap: false,
        itemStyle: { borderRadius: 4 },
        label: { show: false, position: 'center' },
        emphasis: {
          label: {
            show: true,
            fontSize: 16,
            fontWeight: 'bold',
            formatter: '{b}\n{c} errors',
          },
        },
        labelLine: { show: false },
        data: displayData,
      },
    ]
  }

  const option = {
    tooltip: {
      trigger: 'item',
      formatter: '{b}: {c} ({d}%)',
    },
    legend: {
      bottom: '5%',
      left: 'center',
      icon: 'circle',
      itemWidth: 8,
      itemHeight: 8,
      textStyle: { fontSize: 11, color: colors.label },
    },
    series,
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} />
}
