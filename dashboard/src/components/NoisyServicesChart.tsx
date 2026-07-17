import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

const serviceNames = ['api-gateway', 'auth-service', 'payment-processor', 'inventory-db', 'user-service']
const serviceData = [12000, 8500, 5400, 3200, 1800]

export default function NoisyServicesChart() {
  const colors = useChartTheme()
  const option = {
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'shadow' },
      formatter: (p: any) => `${serviceNames[p[0]?.dataIndex]}: ${p[0]?.value?.toLocaleString()} logs`
    },
    grid: { left: '3%', right: '8%', bottom: '3%', top: '3%' },
    xAxis: {
      type: 'value',
      axisLine: { show: false }, axisTick: { show: false },
      axisLabel: { show: false }, splitLine: { show: false }
    },
    yAxis: {
      type: 'category', data: serviceNames,
      axisLine: { show: false }, axisTick: { show: false },
      axisLabel: { show: false }, inverse: true
    },
    series: [
      {
        type: 'bar',
        data: serviceData,
        barWidth: '55%',
        itemStyle: { color: '#f97316', borderRadius: [0, 4, 4, 0] },
        label: {
          show: true, position: 'inside',
          formatter: (p: any) => serviceNames[p.dataIndex],
          fontSize: 13, fontWeight: 600, color: '#fff',
        },
        labelLayout: { dx: 8 },
      },
      {
        type: 'bar',
        data: serviceData,
        barWidth: '55%', barGap: '-100%',
        itemStyle: { color: 'transparent' },
        label: {
          show: true, position: 'right',
          formatter: (p: any) => p.value.toLocaleString(),
          fontSize: 11, fontWeight: 700, color: colors.label,
        },
      },
    ],
  }
  return (
    <ReactECharts
      option={option}
      style={{ height: '100%', width: '100%' }}
    />
  )
}
