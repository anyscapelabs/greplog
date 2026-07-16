import ReactECharts from 'echarts-for-react'

const serviceNames = ['api-gateway', 'auth-service', 'payment', 'db', 'worker']
const healthyData = serviceNames.map(() => Math.floor(Math.random() * 4 + 20))
const degradedData = serviceNames.map(() => Math.floor(Math.random() * 2 + 1))
const downData = serviceNames.map((_, i) => Math.floor(Math.random() * (24 - healthyData[i] - degradedData[i])))

export default function ServiceHealthChart() {
  return (
    <ReactECharts
      option={{
        tooltip: {
          trigger: 'axis',
          axisPointer: { type: 'shadow' },
          formatter: (p: any) => {
            const name = p[0]?.name || ''
            let total = 0
            p.forEach((s: any) => { total += s.value })
            return `<b>${name}</b><br/>` + p.map((s: any) => {
              const pct = total > 0 ? ((s.value / total) * 100).toFixed(0) : 0
              return `${s.marker} ${s.seriesName}: ${s.value}h (${pct}%)`
            }).join('<br/>')
          }
        },
        grid: { left: '3%', right: '10%', bottom: '3%', top: '3%', containLabel: true },
        xAxis: {
          type: 'value', max: 100,
          axisLine: { show: false }, axisTick: { show: false },
          axisLabel: { show: false }, splitLine: { show: false }
        },
        yAxis: {
          type: 'category', data: serviceNames,
          axisLine: { show: false }, axisTick: { show: false },
          axisLabel: { fontSize: 11, color: '#6b7280', margin: 12 }
        },
        series: [
          {
            name: 'Healthy', type: 'bar', stack: 'total', barWidth: '60%',
            data: healthyData,
            itemStyle: { color: '#10b981', borderRadius: 0 },
            label: {
              show: true, position: 'right',
              formatter: (p: any) => p.value > 0 ? `${p.value}h` : '',
              fontSize: 10, color: '#6b7280'
            }
          },
          {
            name: 'Degraded', type: 'bar', stack: 'total', barWidth: '60%',
            data: degradedData,
            itemStyle: { color: '#f59e0b' }
          },
          {
            name: 'Down', type: 'bar', stack: 'total', barWidth: '60%',
            data: downData,
            itemStyle: { color: '#ef4444', borderRadius: [0, 4, 4, 0] }
          }
        ]
      }}
      style={{ height: '100%', width: '100%' }}
    />
  )
}
