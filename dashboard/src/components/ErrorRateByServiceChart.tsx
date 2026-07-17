import ReactECharts from 'echarts-for-react'
import { useChartTheme } from '../utils/useChartTheme.ts'

interface ErrorRateByServiceChartProps {
  metric: string
}

const serviceNames = ['api', 'web', 'db', 'worker']

const countData = [1200, 2400, 9500, 10400]
const rateData = [0.1, 0.3, 2.1, 8.7]

const getColor = (val: number, isRate: boolean) => {
  const v = isRate ? val : val / 12000
  if (v > 0.05) return '#dc2626'
  if (v > 0.01) return '#f97316'
  return '#fde68a'
}

export default function ErrorRateByServiceChart({ metric }: ErrorRateByServiceChartProps) {
  const data = metric === 'count' ? countData : rateData
  const colors = useChartTheme()

  const option = {
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'shadow' },
      formatter: (p: any) => `${serviceNames[p[0]?.dataIndex]}: ${p[0]?.value}${metric === 'rate' ? '%' : ''}`,
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
          itemStyle: { color: getColor(val, metric === 'rate') },
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
          formatter: (p: any) => `${p.value}${metric === 'rate' ? '%' : ''}`,
          fontSize: 11,
          fontWeight: 700,
          color: colors.label,
        },
      },
    ],
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} />
}
