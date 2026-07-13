import ReactECharts from 'echarts-for-react'

export default function ErrorsChart() {
  const times = Array.from({ length: 60 }, (_, i) => {
    const min = String(11 + Math.floor(i / 6)).padStart(2, '0')
    const sec = String((i % 6) * 10).padStart(2, '0')
    return `15:${min}:${sec}`
  })

  // Generate random data with mostly zeros and some spikes for errors
  const data = Array.from({ length: 60 }, () => {
    const r = Math.random()
    if (r > 0.8) return Math.floor(Math.random() * 10 + 2)
    if (r > 0.6) return Math.floor(Math.random() * 5 + 1)
    return 0
  })

  const option = {
    tooltip: { trigger: 'axis' },
    grid: { left: '8%', right: '5%', bottom: '15%', top: '10%' },
    xAxis: {
      type: 'category',
      data: times,
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: {
        fontSize: 10,
        color: '#6b7280',
        hideOverlap: true,
      },
    },
    yAxis: {
      type: 'value',
      splitLine: { lineStyle: { color: '#f3f4f6', width: 1 } },
      axisLine: { show: false },
      axisLabel: { fontSize: 10, color: '#6b7280' },
    },
    series: [
      {
        type: 'bar',
        data: data,
        itemStyle: { color: '#f87171' },
        barWidth: '70%',
      },
    ],
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} />
}
