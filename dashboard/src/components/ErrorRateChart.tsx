import ReactECharts from 'echarts-for-react'

export default function ErrorRateChart() {
  const times = Array.from({ length: 60 }, (_, i) => {
    const min = String(11 + Math.floor(i / 6)).padStart(2, '0')
    const sec = String((i % 6) * 10).padStart(2, '0')
    return `15:${min}:${sec}`
  })

  const data = Array.from({ length: 60 }, () => Math.random() * 8 + 0.5)

  const option = {
    tooltip: { trigger: 'axis' },
    grid: { left: '8%', right: '5%', bottom: '15%', top: '10%' },
    xAxis: {
      type: 'category',
      data: times,
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: { fontSize: 10, color: '#6b7280', hideOverlap: true },
    },
    yAxis: {
      type: 'value',
      splitLine: { lineStyle: { color: '#f3f4f6', width: 1 } },
      axisLine: { show: false },
      axisLabel: { fontSize: 10, color: '#6b7280' },
    },
    series: [
      {
        type: 'line',
        smooth: true,
        symbol: 'none',
        data: data,
        lineStyle: { width: 2, color: '#f87171' },
        itemStyle: { color: '#f87171' },
        areaStyle: {
          color: {
            type: 'linear',
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: 'rgba(248, 113, 113, 0.3)' },
              { offset: 1, color: 'rgba(248, 113, 113, 0.05)' },
            ],
          },
        },
      },
    ],
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} />
}
