import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

const serviceNames = ['db', 'api-gateway', 'worker', 'auth-service', 'payment']
const responseValues = [15, 45, 80, 120, 250]

export default function AvgResponseTimeChart() {
  const colors = useChartTheme()
  const option = {
    tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
    grid: { left: '3%', right: '8%', bottom: '3%', top: '3%' },
    xAxis: {
      type: 'value',
      axisLine: { show: false }, axisTick: { show: false },
      axisLabel: { show: false }, splitLine: { show: false }
    },
    yAxis: {
      type: 'category', data: serviceNames,
      axisLine: { show: false }, axisTick: { show: false },
      axisLabel: { show: false }
    },
    series: [
      {
        type: 'bar',
        data: responseValues,
        barWidth: '50%',
        itemStyle: { color: '#14b8a6', borderRadius: [0, 4, 4, 0] },
        label: {
          show: true, position: 'inside',
          formatter: (p: any) => serviceNames[p.dataIndex],
          fontSize: 13, fontWeight: 600, color: '#fff',
        },
        labelLayout: { dx: 8 },
      },
      {
        type: 'bar',
        data: responseValues.map((v) => v),
        barWidth: '50%', barGap: '-100%',
        itemStyle: { color: 'transparent' },
        label: {
          show: true, position: 'right',
          formatter: (p: any) => `${p.value}ms`,
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
