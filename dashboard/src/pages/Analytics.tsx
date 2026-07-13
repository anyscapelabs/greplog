import { useState } from 'react'
import { LuRefreshCw, LuChevronDown, LuServer } from 'react-icons/lu'
import ReactECharts from 'echarts-for-react'

const generateData = (base: number, variance: number, min: number = 0) => {
  let current = base;
  return Array.from({ length: 120 }, () => {
    current += (Math.random() - 0.5) * variance;
    if (current < min) current = min;
    return current;
  });
};

const metricsData = [
  { title: 'Requests', value: '1.2M', color: '#3b82f6', rgb: '59, 130, 246', data: generateData(200, 50) },
  { title: 'Error rate', value: '0.12%', color: '#dc2626', rgb: '220, 38, 38', data: generateData(1, 0.5) },
  { title: 'P.95 latency', value: '145ms', color: '#d97706', rgb: '217, 119, 6', data: generateData(145, 10) },
  { title: 'Request throughput', value: '2.4k/s', color: '#16a34a', rgb: '22, 163, 74', data: generateData(2400, 200) },
  { title: 'Active services', value: '42', color: '#2563eb', rgb: '37, 99, 235', data: generateData(42, 2) },
  { title: 'Trace volume', value: '840GB', color: '#8b5cf6', rgb: '139, 92, 246', data: generateData(800, 40) },
]

const bottomChartTimes = Array.from({ length: 96 }, (_, i) => {
  const h = String(Math.floor(i / 4)).padStart(2, '0')
  const m = String((i % 4) * 15).padStart(2, '0')
  return `${h}:${m}`
});
const ingestionData = Array.from({ length: 96 }, () => Math.floor(Math.random() * 50 + 10));
const errorData = Array.from({ length: 96 }, () => Math.random() * 2);
const latencyP50 = Array.from({ length: 96 }, () => 20 + Math.random() * 10);
const latencyP90 = Array.from({ length: 96 }, () => 50 + Math.random() * 20);
const latencyP99 = Array.from({ length: 96 }, () => 100 + Math.random() * 40);

const statusCodesData = [
  { value: 4200, name: '2xx', itemStyle: { color: '#10b981' } },
  { value: 300, name: '3xx', itemStyle: { color: '#3b82f6' } },
  { value: 150, name: '4xx', itemStyle: { color: '#f59e0b' } },
  { value: 20, name: '5xx', itemStyle: { color: '#ef4444' } }
];

const serviceHealthNames = ['api-gateway', 'auth-service', 'payment', 'db', 'worker'];
const serviceHealthyData = serviceHealthNames.map(() => Math.floor(Math.random() * 4 + 20));
const serviceDegradedData = serviceHealthNames.map(() => Math.floor(Math.random() * 2 + 1));
const serviceDownData = serviceHealthNames.map((_, i) => Math.floor(Math.random() * (24 - serviceHealthyData[i] - serviceDegradedData[i])));

const noisyServiceNames = ['api-gateway', 'auth-service', 'payment-processor', 'inventory-db', 'user-service'];
const noisyServiceData = [12000, 8500, 5400, 3200, 1800];

const severityData = [
  { value: 75000, name: 'INFO', itemStyle: { color: '#3b82f6' } },
  { value: 12000, name: 'WARN', itemStyle: { color: '#f59e0b' } },
  { value: 3000, name: 'ERROR', itemStyle: { color: '#ef4444' } },
  { value: 500, name: 'DEBUG', itemStyle: { color: '#8b5cf6' } }
];

const cpuMemTimes = Array.from({ length: 60 }, (_, i) => `10:${String(i).padStart(2, '0')}`);
const cpuData = Array.from({ length: 60 }, () => 40 + Math.random() * 30);
const memData = Array.from({ length: 60 }, () => 60 + Math.random() * 20);

const responseTimeNames = ['db', 'api-gateway', 'worker', 'auth-service', 'payment'];
const responseTimeValues = [15, 45, 80, 120, 250];

const timeRanges = ['Last 15 min', 'Last 1 hour', 'Last 6 hours', 'Last 24 hours', 'Last 7 days', 'Custom']
const services = ['All Services', 'web', 'api', 'db', 'worker']

export default function Analytics() {
  const [spinning, setSpinning] = useState(false)
  const [timeRange, setTimeRange] = useState('Last 15 min')
  const [timeOpen, setTimeOpen] = useState(false)
  const [service, setService] = useState('All Services')
  const [serviceOpen, setServiceOpen] = useState(false)
  const [autoRefresh, setAutoRefresh] = useState('Off')
  const [refreshOpen, setRefreshOpen] = useState(false)

  return (
    <div className="flex flex-col h-full">
      <div
        className="flex items-center px-4 h-12 shrink-0 border-b gap-3"
        style={{
          backgroundColor: 'var(--bg-secondary)',
          borderColor: 'var(--border-primary)',
        }}
      >
        <span className="text-2xl font-semibold flex items-center gap-2">
          <span style={{ color: 'var(--text-secondary)' }}>Grep</span>
          <span className="text-text-primary">Analytics</span>
        </span>
        <div className="ml-auto flex items-center gap-2">
          <button
            className="flex items-center gap-1.5 px-2 py-1 text-sm text-text-primary hover:bg-gray-100 transition-colors"
            style={{
              borderColor: 'var(--border-primary)',
              borderWidth: 1,
            }}
            onClick={() => {
              setSpinning(true)
              setTimeout(() => setSpinning(false), 600)
            }}
          >
            <LuRefreshCw className={`size-3.5 ${spinning ? 'animate-spin' : ''}`} />
            Refresh
          </button>
          <div className="relative">
            <button
              className="flex items-center gap-1.5 px-2 py-1 text-sm hover:bg-gray-100 transition-colors"
              style={{
                borderColor: 'var(--border-primary)',
                borderWidth: 1,
              }}
              onClick={() => setServiceOpen(!serviceOpen)}
            >
              <LuServer className="size-3.5" style={{ color: 'var(--text-secondary)' }} />
              <span className="text-text-primary">{service}</span>
              <LuChevronDown className="size-3.5" style={{ color: 'var(--text-secondary)' }} />
            </button>
            {serviceOpen && (
              <div
                className="absolute top-full right-0 mt-1 py-1 min-w-32 rounded border bg-white shadow-md z-50"
                style={{ borderColor: 'var(--border-primary)' }}
              >
                {services.map((s) => (
                  <button
                    key={s}
                    className={`w-full text-left px-3 py-1.5 text-sm transition-colors ${
                      s === service ? 'text-text-primary bg-gray-100 font-medium' : 'text-text-primary hover:bg-gray-50'
                    }`}
                    onClick={() => { setService(s); setServiceOpen(false) }}
                  >
                    {s}
                  </button>
                ))}
              </div>
            )}
          </div>
          <div className="relative">
            <button
              className="flex items-center gap-1.5 px-2 py-1 text-sm hover:bg-gray-100 transition-colors"
              style={{
                borderColor: 'var(--border-primary)',
                borderWidth: 1,
              }}
              onClick={() => setRefreshOpen(!refreshOpen)}
            >
              <span className="text-text-primary text-sm">Auto refresh</span>
              {autoRefresh !== 'Off' && (
                <span className="flex items-center justify-center px-1.5 py-0.5 text-xs text-text-primary bg-gray-100 rounded">
                  {autoRefresh}
                </span>
              )}
            </button>
            {refreshOpen && (
              <div
                className="absolute top-full right-0 mt-1 py-1 min-w-16 rounded border bg-white shadow-md z-50"
                style={{ borderColor: 'var(--border-primary)' }}
              >
                {['Off', '10s', '30s', '1m', '5m'].map((opt) => (
                  <button
                    key={opt}
                    className={`w-full text-left px-3 py-1 text-xs transition-colors ${
                      opt === autoRefresh ? 'text-text-primary bg-gray-100 font-medium' : 'text-text-primary hover:bg-gray-50'
                    }`}
                    onClick={() => { setAutoRefresh(opt); setRefreshOpen(false) }}
                  >
                    {opt}
                  </button>
                ))}
              </div>
            )}
          </div>
          <div className="relative">
            <button
              className="flex items-center gap-1.5 px-2 py-1 text-sm text-text-primary hover:bg-gray-100 transition-colors"
              style={{
                borderColor: 'var(--border-primary)',
                borderWidth: 1,
              }}
              onClick={() => setTimeOpen(!timeOpen)}
            >
              <span>{timeRange}</span>
              <LuChevronDown className="size-3.5" style={{ color: 'var(--text-secondary)' }} />
            </button>
            {timeOpen && (
              <div
                className="absolute top-full right-0 mt-1 py-1 min-w-40 rounded border bg-white shadow-md z-50"
                style={{ borderColor: 'var(--border-primary)' }}
              >
                {timeRanges.map((range) => (
                  <button
                    key={range}
                    className={`w-full text-left px-3 py-1.5 text-sm transition-colors ${
                      range === timeRange ? 'text-text-primary bg-gray-100 font-medium' : 'text-text-primary hover:bg-gray-50'
                    }`}
                    onClick={() => { setTimeRange(range); setTimeOpen(false) }}
                  >
                    {range}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-0.5">
        <div className="grid grid-cols-6 gap-0.5">
          {metricsData.map((metric) => {
            const option = {
              grid: { left: -1, right: -1, top: 60, bottom: 0 },
              xAxis: { type: 'category', show: false, boundaryGap: false },
              yAxis: {
                type: 'value',
                show: false,
                min: (value: any) => {
                  const range = value.max - value.min || value.max * 0.1 || 1
                  return Math.max(0, value.min - range * 0.2)
                },
                max: (value: any) => {
                  const range = value.max - value.min || value.max * 0.1 || 1
                  return value.max + range * 0.2
                }
              },
              series: [
                {
                  data: metric.data,
                  type: 'line',
                  smooth: false,
                  showSymbol: false,
                  lineStyle: {
                    color: metric.color,
                    width: 1.5,
                  },
                  areaStyle: {
                    color: `rgba(${metric.rgb}, 0.5)`,
                  },
                },
              ],
              tooltip: { show: false },
            }

            return (
              <div
                key={metric.title}
                className="rounded border h-36 relative overflow-hidden flex flex-col p-3"
                style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}
              >
                <div className="z-10 flex flex-col relative">
                  <span className="text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>{metric.title}</span>
                  <span className="text-3xl font-bold mt-1 tracking-tight" style={{ color: 'var(--text-primary)' }}>{metric.value}</span>
                </div>
                <div className="absolute inset-0 z-0 pointer-events-none">
                  <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge={true} />
                </div>
              </div>
            )
          })}
        </div>
        <div className="rounded border h-80 flex flex-col mt-0.5" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
          <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
            <span className="text-sm font-semibold text-text-primary">Log Ingestion Over Time</span>
            <div className="flex items-center gap-2">
              <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                Sum <LuChevronDown className="size-3" />
              </button>
            </div>
          </div>
          <div className="flex-1 p-1">
            <ReactECharts 
              option={{
                tooltip: { trigger: 'axis' },
                grid: { left: '3%', right: '2%', bottom: '16%', top: '8%' },
                xAxis: { type: 'category', data: bottomChartTimes, axisLine: { show: false }, axisTick: { show: false }, axisLabel: { fontSize: 10, color: '#6b7280', interval: 5 } },
                yAxis: { type: 'value', splitLine: { lineStyle: { color: '#e5e7eb' } }, axisLine: { show: false }, axisLabel: { fontSize: 10, color: '#6b7280' } },
                series: [{ type: 'bar', data: ingestionData, itemStyle: { color: '#3b82f6' }, barWidth: '60%' }]
              }} 
              style={{ height: '100%', width: '100%' }} 
            />
          </div>
        </div>
        <div className="grid grid-cols-2 gap-0.5 mt-0.5">
          <div className="rounded border h-80 flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
            <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
              <span className="text-sm font-semibold text-text-primary">Error Rate Over Time</span>
              <div className="flex items-center gap-2">
                <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                  Average <LuChevronDown className="size-3" />
                </button>
              </div>
            </div>
            <div className="flex-1 p-1">
              <ReactECharts 
                option={{
                  tooltip: { trigger: 'axis' },
                  grid: { left: '3%', right: '2%', bottom: '16%', top: '8%' },
                  xAxis: { type: 'category', data: bottomChartTimes, axisLine: { show: false }, axisTick: { show: false }, axisLabel: { fontSize: 10, color: '#6b7280', interval: 7 } },
                  yAxis: { type: 'value', splitLine: { lineStyle: { color: '#e5e7eb' } }, axisLine: { show: false }, axisLabel: { fontSize: 10, color: '#6b7280' } },
                  series: [{
                    type: 'line', smooth: false, symbol: 'none',
                    data: errorData,
                    itemStyle: { color: '#dc2626' },
                    areaStyle: { color: { type: 'linear', x: 0, y: 0, x2: 0, y2: 1, colorStops: [{ offset: 0, color: 'rgba(220, 38, 38, 0.4)' }, { offset: 1, color: 'rgba(220, 38, 38, 0.05)' }] } }
                  }]
                }} 
                style={{ height: '100%', width: '100%' }} 
              />
            </div>
          </div>

          <div className="rounded border h-80 flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
            <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
              <span className="text-sm font-semibold text-text-primary">Latency Percentiles</span>
              <div className="flex items-center gap-2">
                <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                  P50, P90, P99 <LuChevronDown className="size-3" />
                </button>
              </div>
            </div>
            <div className="flex-1 p-1">
              <ReactECharts 
                option={{
                  tooltip: { trigger: 'axis' },
                  legend: { data: ['P50', 'P90', 'P99'], bottom: 0, icon: 'circle', itemWidth: 8, itemHeight: 8, textStyle: { fontSize: 10, color: '#6b7280' } },
                  grid: { left: '3%', right: '2%', bottom: '24%', top: '8%' },
                  xAxis: { type: 'category', data: bottomChartTimes, axisLine: { show: false }, axisTick: { show: false }, axisLabel: { fontSize: 10, color: '#6b7280', interval: 7 } },
                  yAxis: { type: 'value', splitLine: { lineStyle: { color: '#e5e7eb' } }, axisLine: { show: false }, axisLabel: { fontSize: 10, color: '#6b7280' } },
                  series: [
                    { name: 'P50', type: 'line', smooth: false, symbol: 'none', data: latencyP50, itemStyle: { color: '#10b981' } },
                    { name: 'P90', type: 'line', smooth: false, symbol: 'none', data: latencyP90, itemStyle: { color: '#f59e0b' } },
                    { name: 'P99', type: 'line', smooth: false, symbol: 'none', data: latencyP99, itemStyle: { color: '#ef4444' } }
                  ]
                }} 
                style={{ height: '100%', width: '100%' }} 
              />
            </div>
          </div>
        </div>
        <div className="grid grid-cols-3 gap-0.5 mt-0.5 pb-0.5">
          <div className="rounded border h-80 flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
            <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
              <span className="text-sm font-semibold text-text-primary">Service Health</span>
              <div className="flex items-center gap-2">
                <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                  Current <LuChevronDown className="size-3" />
                </button>
              </div>
            </div>
            <div className="flex-1 p-1">
              <ReactECharts 
                option={{
                  tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' }, formatter: (p: any) => {
                    const name = p[0]?.name || ''
                    let total = 0
                    p.forEach((s: any) => { total += s.value })
                    return `<b>${name}</b><br/>` + p.map((s: any) => {
                      const pct = total > 0 ? ((s.value / total) * 100).toFixed(0) : 0
                      return `${s.marker} ${s.seriesName}: ${s.value}h (${pct}%)`
                    }).join('<br/>')
                  }},
                  grid: { left: '3%', right: '10%', bottom: '3%', top: '3%', containLabel: true },
                  xAxis: { type: 'value', max: 100, axisLine: { show: false }, axisTick: { show: false }, axisLabel: { show: false }, splitLine: { show: false } },
                  yAxis: { type: 'category', data: serviceHealthNames, axisLine: { show: false }, axisTick: { show: false }, axisLabel: { fontSize: 11, color: '#6b7280', margin: 12 } },
                  series: [
                    { name: 'Healthy', type: 'bar', stack: 'total', barWidth: '60%', data: serviceHealthyData, itemStyle: { color: '#10b981', borderRadius: 0 }, label: { show: true, position: 'right', formatter: (p: any) => p.value > 0 ? `${p.value}h` : '', fontSize: 10, color: '#6b7280' } },
                    { name: 'Degraded', type: 'bar', stack: 'total', barWidth: '60%', data: serviceDegradedData, itemStyle: { color: '#f59e0b' } },
                    { name: 'Down', type: 'bar', stack: 'total', barWidth: '60%', data: serviceDownData, itemStyle: { color: '#ef4444', borderRadius: [0, 4, 4, 0] } }
                  ]
                }} 
                style={{ height: '100%', width: '100%' }} 
              />
            </div>
          </div>

          <div className="rounded border h-80 flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
            <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
              <span className="text-sm font-semibold text-text-primary">Status Codes</span>
              <div className="flex items-center gap-2">
                <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                  Sum <LuChevronDown className="size-3" />
                </button>
              </div>
            </div>
            <div className="flex-1 p-1">
              <ReactECharts 
                option={{
                  tooltip: { trigger: 'item' },
                  legend: { bottom: 0, icon: 'circle', itemWidth: 8, itemHeight: 8, textStyle: { fontSize: 10, color: '#6b7280' } },
                  series: [
                    {
                      type: 'pie',
                      radius: ['40%', '70%'],
                      center: ['50%', '45%'],
                      avoidLabelOverlap: false,
                      itemStyle: { borderRadius: 4, borderColor: '#fff', borderWidth: 2 },
                      label: { show: false, position: 'center' },
                      emphasis: { label: { show: true, fontSize: '16', fontWeight: 'bold' } },
                      labelLine: { show: false },
                      data: statusCodesData
                    }
                  ]
                }} 
                style={{ height: '100%', width: '100%' }} 
              />
            </div>
          </div>

          <div className="rounded border h-80 flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
            <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
              <span className="text-sm font-semibold text-text-primary">Top Noisy Services</span>
              <div className="flex items-center gap-2">
                <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                  Logs Generated <LuChevronDown className="size-3" />
                </button>
              </div>
            </div>
            <div className="flex-1 p-1">
              <ReactECharts 
                option={{
                  tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' }, formatter: (p: any) => `${noisyServiceNames[p[0]?.dataIndex]}: ${p[0]?.value?.toLocaleString()} logs` },
                  grid: { left: '3%', right: '8%', bottom: '3%', top: '3%' },
                  xAxis: { type: 'value', axisLine: { show: false }, axisTick: { show: false }, axisLabel: { show: false }, splitLine: { show: false } },
                  yAxis: { type: 'category', data: noisyServiceNames, axisLine: { show: false }, axisTick: { show: false }, axisLabel: { show: false }, inverse: true },
                  series: [
                    {
                      type: 'bar',
                      data: noisyServiceData,
                      barWidth: '55%',
                      itemStyle: { color: '#f97316', borderRadius: [0, 4, 4, 0] },
                      label: {
                        show: true,
                        position: 'inside',
                        formatter: (p: any) => noisyServiceNames[p.dataIndex],
                        fontSize: 13,
                        fontWeight: 600,
                        color: '#fff',
                      },
                      labelLayout: { dx: 8 },
                    },
                    {
                      type: 'bar',
                      data: noisyServiceData,
                      barWidth: '55%',
                      barGap: '-100%',
                      itemStyle: { color: 'transparent' },
                      label: {
                        show: true,
                        position: 'right',
                        formatter: (p: any) => p.value.toLocaleString(),
                        fontSize: 11,
                        fontWeight: 700,
                        color: '#6b7280',
                      },
                    },
                  ],
                }} 
                style={{ height: '100%', width: '100%' }} 
              />
            </div>
          </div>
        </div>
        <div className="grid grid-cols-3 gap-0.5 mt-0.5 pb-0.5">
          <div className="rounded border h-80 flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
            <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
              <span className="text-sm font-semibold text-text-primary">Log Severity Distribution</span>
              <div className="flex items-center gap-2">
                <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                  Current <LuChevronDown className="size-3" />
                </button>
              </div>
            </div>
            <div className="flex-1 p-1">
              <ReactECharts 
                option={{
                  tooltip: { trigger: 'item' },
                  legend: { bottom: 0, icon: 'circle', itemWidth: 8, itemHeight: 8, textStyle: { fontSize: 10, color: '#6b7280' } },
                  series: [
                    {
                      type: 'pie',
                      radius: ['40%', '70%'],
                      center: ['50%', '45%'],
                      avoidLabelOverlap: false,
                      itemStyle: { borderRadius: 4, borderColor: '#fff', borderWidth: 2 },
                      label: { show: false, position: 'center' },
                      emphasis: { label: { show: true, fontSize: '16', fontWeight: 'bold' } },
                      labelLine: { show: false },
                      data: severityData
                    }
                  ]
                }} 
                style={{ height: '100%', width: '100%' }} 
              />
            </div>
          </div>

          <div className="rounded border h-80 flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
            <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
              <span className="text-sm font-semibold text-text-primary">System Metrics</span>
              <div className="flex items-center gap-2">
                <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                  Last 24h <LuChevronDown className="size-3" />
                </button>
              </div>
            </div>
            <div className="flex-1 grid grid-cols-2 grid-rows-2 gap-1 p-2">
              {[
                { label: 'CPU', value: '47%', data: cpuData, color: '#8b5cf6' },
                { label: 'Memory', value: '72%', data: memData, color: '#ec4899' },
                { label: 'Disk I/O', value: '234 MB/s', data: cpuData.map(() => Math.random() * 100 + 50), color: '#14b8a6' },
                { label: 'Network', value: '1.2 Gbps', data: cpuData.map(() => Math.random() * 80 + 20), color: '#f97316' },
              ].map((m) => (
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
                        yAxis: { type: 'value', show: false, min: (value: any) => { const r = value.max - value.min || value.max * 0.1 || 1; return Math.max(0, value.min - r * 0.3) }, max: (value: any) => { const r = value.max - value.min || value.max * 0.1 || 1; return value.max + r * 0.3 } },
                        series: [{ type: 'line', data: m.data, smooth: false, showSymbol: false, lineStyle: { color: m.color, width: 1.5 }, areaStyle: { color: { type: 'linear', x: 0, y: 0, x2: 0, y2: 1, colorStops: [{ offset: 0, color: `${m.color}55` }, { offset: 1, color: `${m.color}11` }] } } }],
                        tooltip: { show: false },
                      }}
                      style={{ height: '100%', width: '100%' }}
                    />
                  </div>
                </div>
              ))}
            </div>
          </div>

          <div className="rounded border h-80 flex flex-col" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
            <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
              <span className="text-sm font-semibold text-text-primary">Avg Response Time</span>
              <div className="flex items-center gap-2">
                <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors">
                  By Service <LuChevronDown className="size-3" />
                </button>
              </div>
            </div>
            <div className="flex-1 p-1">
              <ReactECharts 
                option={{
                  tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
                  grid: { left: '3%', right: '8%', bottom: '3%', top: '3%' },
                  xAxis: { type: 'value', axisLine: { show: false }, axisTick: { show: false }, axisLabel: { show: false }, splitLine: { show: false } },
                  yAxis: { type: 'category', data: responseTimeNames, axisLine: { show: false }, axisTick: { show: false }, axisLabel: { show: false } },
                  series: [
                    {
                      type: 'bar',
                      data: responseTimeValues,
                      barWidth: '50%',
                      itemStyle: { color: '#14b8a6', borderRadius: [0, 4, 4, 0] },
                      label: {
                        show: true,
                        position: 'inside',
                        formatter: (p: any) => responseTimeNames[p.dataIndex],
                        fontSize: 13,
                        fontWeight: 600,
                        color: '#fff',
                      },
                      labelLayout: { dx: 8 },
                    },
                    {
                      type: 'bar',
                      data: responseTimeValues.map((v) => v),
                      barWidth: '50%',
                      barGap: '-100%',
                      itemStyle: { color: 'transparent' },
                      label: {
                        show: true,
                        position: 'right',
                        formatter: (p: any) => `${p.value}ms`,
                        fontSize: 11,
                        fontWeight: 700,
                        color: '#6b7280',
                      },
                    },
                  ],
                }} 
                style={{ height: '100%', width: '100%' }} 
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
