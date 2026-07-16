import ReactECharts from 'echarts-for-react'

const cpuData = Array.from({ length: 60 }, () => 40 + Math.random() * 30)
const memData = Array.from({ length: 60 }, () => 60 + Math.random() * 20)

const metrics = [
  { label: 'CPU', value: '47%', data: cpuData, color: '#8b5cf6' },
  { label: 'Memory', value: '72%', data: memData, color: '#ec4899' },
  { label: 'Disk I/O', value: '234 MB/s', data: cpuData.map(() => Math.random() * 100 + 50), color: '#14b8a6' },
  { label: 'Network', value: '1.2 Gbps', data: cpuData.map(() => Math.random() * 80 + 20), color: '#f97316' },
]

export default function SystemMetricsChart() {
  return (
    <div className="flex-1 grid grid-cols-2 grid-rows-2 gap-1 p-2">
      {metrics.map((m) => (
        <div key={m.label} className="flex flex-col">
          <div className="flex items-center justify-between px-1">
            <span className="text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>{m.label}</span>
            <span className="text-xs font-bold text-text-primary">{m.value}</span>
          </div>
          <div className="flex-1 min-h-0">
            <ReactECharts
              option={{
                grid: { left: 0, right: 0, top: 2, bottom: 0 },
                xAxis: { type: 'category', show: false, boundaryGap: false },
                yAxis: {
                  type: 'value', show: false,
                  min: (value: any) => { const r = value.max - value.min || value.max * 0.1 || 1; return Math.max(0, value.min - r * 0.3) },
                  max: (value: any) => { const r = value.max - value.min || value.max * 0.1 || 1; return value.max + r * 0.3 }
                },
                series: [{
                  type: 'line', data: m.data, smooth: false, showSymbol: false,
                  lineStyle: { color: m.color, width: 1.5 },
                  areaStyle: {
                    color: {
                      type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
                      colorStops: [
                        { offset: 0, color: `${m.color}55` },
                        { offset: 1, color: `${m.color}11` }
                      ]
                    }
                  }
                }],
                tooltip: { show: false },
              }}
              style={{ height: '100%', width: '100%' }}
            />
          </div>
        </div>
      ))}
    </div>
  )
}
