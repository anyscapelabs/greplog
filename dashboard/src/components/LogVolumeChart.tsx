import ReactECharts from 'echarts-for-react'

export default function LogVolumeChart() {
  const times = Array.from({ length: 60 }, (_, i) => {
    const min = String(11 + Math.floor(i / 6)).padStart(2, '0')
    const sec = String((i % 6) * 10).padStart(2, '0')
    return `15:${min}:${sec}`
  })

  // Generate random data for Total Requests
  const data = Array.from({ length: 60 }, () => Math.floor(Math.random() * 400 + 200))

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
        type: 'line',
        smooth: false,
        symbol: 'none',
        data: data,
        itemStyle: { color: '#4ade80' },
        lineStyle: { width: 2, color: '#4ade80' },
        areaStyle: {
          color: {
            type: 'linear',
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: 'rgba(74, 222, 128, 0.4)' },
              { offset: 1, color: 'rgba(74, 222, 128, 0.05)' },
            ],
          },
        },
      },
    ],
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} />
}
