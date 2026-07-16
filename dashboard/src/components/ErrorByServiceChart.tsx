import ReactECharts from 'echarts-for-react'

export default function ErrorByServiceChart() {
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
      textStyle: { fontSize: 11, color: '#6b7280' },
    },
    series: [
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
        data: [
          { value: 234, name: 'web', itemStyle: { color: '#fb923c' } },
          { value: 567, name: 'api', itemStyle: { color: '#f87171' } },
          { value: 89, name: 'db', itemStyle: { color: '#fbbf24' } },
          { value: 123, name: 'worker', itemStyle: { color: '#a78bfa' } },
        ],
      },
    ],
  }

  return <ReactECharts option={option} style={{ height: '100%', width: '100%' }} />
}
