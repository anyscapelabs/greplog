import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

interface RequestsByServiceChartProps {
  metric: string
}

const serviceNames = ['api', 'web', 'db', 'worker']
const serviceColors = ['#3b82f6', '#22c55e', '#d97706', '#ef4444']

const countData = [1200000, 890000, 450000, 120000]
const rateData = [2500, 1850, 940, 250]

export default function RequestsByServiceChart({ metric }: RequestsByServiceChartProps) {
  const data = metric === 'count' ? countData : rateData
  const colors = useChartTheme()

  const option = {
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'shadow' },
      formatter: (p: any) => `${serviceNames[p[0]?.dataIndex]}: ${p[0]?.value?.toLocaleString()}${metric === 'count' ? '' : '/s'}`,
    },
    grid: { left: '3%', right: '8%', bottom: '3%', top: '3%' },
    xAxis: {
      type: 'value',
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: { show: false },
      splitLine: { show: false },
    },
    yAxis: {
      type: 'category',
      data: serviceNames,
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: { show: false },
      inverse: true,
    },
    series: [
      {
        type: 'bar',
        data: data.map((val, i) => ({
          value: val,
          itemStyle: { color: serviceColors[i] },
        })),
        barWidth: '55%',
        itemStyle: { borderRadius: [0, 4, 4, 0] },
        label: {
          show: true,
          position: 'inside',
          formatter: (p: any) => serviceNames[p.dataIndex],
          fontSize: 13,
          fontWeight: 600,
          color: '#fff',
        },
        labelLayout: { dx: 8 },
      },
      {
        type: 'bar',
        data,
        barWidth: '55%',
        barGap: '-100%',
        itemStyle: { color: 'transparent' },
        label: {
          show: true,
          position: 'right',
          formatter: (p: any) => p.value.toLocaleString(),
          fontSize: 11,
          fontWeight: 700,
          color: colors.label,
        },
      },
    ],
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} />
}
