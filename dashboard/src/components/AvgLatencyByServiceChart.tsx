import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

interface AvgLatencyByServiceChartProps {
  metric: string
}

const serviceNames = ['api', 'web', 'db', 'worker']

const dataMap: Record<string, number[]> = {
  avg: [45, 52, 230, 890],
  p50: [38, 44, 195, 756],
  p95: [95, 110, 490, 1900],
  p99: [180, 210, 930, 3600],
}

const getColor = (val: number) => {
  if (val > 500) return '#dc2626'
  if (val > 100) return '#d97706'
  return '#f59e0b'
}

export default function AvgLatencyByServiceChart({ metric }: AvgLatencyByServiceChartProps) {
  const data = dataMap[metric] || dataMap.avg
  const colors = useChartTheme()

  const option = {
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'shadow' },
      formatter: (p: any) => `${serviceNames[p[0]?.dataIndex]}: ${p[0]?.value}ms`,
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
        data: data.map((val) => ({
          value: val,
          itemStyle: { color: getColor(val) },
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
          formatter: (p: any) => `${p.value}ms`,
          fontSize: 11,
          fontWeight: 700,
          color: colors.label,
        },
      },
    ],
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} />
}
