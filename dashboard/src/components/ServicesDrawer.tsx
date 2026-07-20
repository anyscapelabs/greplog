import { useMemo } from 'react'
import { LuX } from 'react-icons/lu'
import { useNavigate } from 'react-router-dom'
import ReactECharts from 'echarts-for-react'
import ServicesIcon from '../icons/ServicesIcon.tsx'
import { useChartTheme } from '../utils/useChartTheme.ts'
import HealthTimeline from './HealthTimeline.tsx'

const generateData = (base: number, variance: number, min: number = 0) => {
  let current = base
  return Array.from({ length: 60 }, () => {
    current += (Math.random() - 0.5) * variance
    if (current < min) current = min
    return current
  })
}

const metrics = [
  { label: 'Req/s', base: 2500, variance: 500 },
  { label: 'Reqs', base: 4500000, variance: 200000, min: 4000000 },
  { label: 'Avg Latency', base: 45, variance: 15, suffix: 'ms' },
  { label: 'P95 Latency', base: 120, variance: 40, suffix: 'ms' },
  { label: 'Error Rate', base: 1.2, variance: 0.5, suffix: '%' },
]

function getMetricColor(value: number, index: number): string {
  if (index === 2) {
    if (value < 50) return 'green'
    if (value < 100) return 'orange'
    return 'red'
  }
  if (index === 3) {
    if (value < 100) return 'green'
    if (value < 200) return 'orange'
    return 'red'
  }
  if (index === 4) {
    if (value < 1) return 'blue'
    if (value < 3) return 'orange'
    return 'red'
  }
  return ['blue', 'green'][index] || 'blue'
}

const formatValue = (val: number, suffix?: string) => {
  if (val >= 1000000) return `${(val / 1000000).toFixed(1)}M${suffix || ''}`
  if (val >= 1000) return `${(val / 1000).toFixed(0)}k${suffix || ''}`
  return `${val.toFixed(1)}${suffix || ''}`
}

interface ServicesDrawerProps {
  open: boolean
  onClose: () => void
  service: { service: string } | null
}

export default function ServicesDrawer({ open, onClose, service }: ServicesDrawerProps) {
  const navigate = useNavigate()
  const colors = useChartTheme()
  const sparklines = useMemo(() => metrics.map((m) => generateData(m.base, m.variance, 'min' in m ? m.min : 0)), [])



  if (!open) return null

  return (
    <>
      <div className="fixed inset-0 bg-black/30 z-40" onClick={onClose} />
      <div
        className="fixed top-0 right-0 h-full w-[1200px] border-l shadow-xl z-50 flex flex-col animate-in slide-in-from-right"
        style={{
          backgroundColor: 'var(--bg-secondary)',
          borderColor: 'var(--border-primary)',
        }}
      >
        <div className="flex items-center justify-between px-4 h-12 border-b shrink-0" style={{ borderColor: 'var(--border-primary)' }}>
          <span className="text-sm font-semibold text-text-primary flex items-center gap-2">
            <ServicesIcon className="size-4" style={{ color: 'var(--text-secondary)' }} />
            Service: {service?.service}
          </span>
          <button
            className="flex items-center justify-center size-7 rounded hover:bg-[var(--hover-bg)] transition-colors"
            onClick={onClose}
          >
            <LuX className="size-4" style={{ color: 'var(--text-secondary)' }} />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto p-4">
          <div className="flex gap-3 mb-6">
            {metrics.map((m, i) => {
              const data = sparklines[i]
              const current = data[data.length - 1]
              const colorKey = getMetricColor(current, i) as keyof typeof colors
              const color = colors[colorKey]
              const option = {
                grid: { left: -1, right: -1, top: 52, bottom: 0 },
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
                  },
                },
                series: [
                  {
                    data,
                    type: 'line',
                    smooth: false,
                    showSymbol: false,
                    lineStyle: { color, width: 1.5 },
                    areaStyle: { color: `${color}80` },
                  },
                ],
                tooltip: { show: false },
              }
              return (
                <div
                  key={i}
                  className="flex-1 rounded border h-[120px] relative overflow-hidden flex flex-col p-3"
                  style={{ backgroundColor: `${color}15`, borderColor: 'var(--border-primary)' }}
                >
                  <div className="z-10 flex flex-col relative">
                    <span className="text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>{m.label}</span>
                    <span className="text-xl font-bold mt-0.5 tracking-tight" style={{ color: 'var(--text-primary)' }}>{formatValue(current, m.suffix)}</span>
                  </div>
                  <div className="absolute inset-0 z-0 pointer-events-none">
                    <ReactECharts option={option} style={{ height: '100%', width: '100%' }} notMerge />
                  </div>
                </div>
              )
            })}
          </div>
          <div className="flex flex-col gap-2">
            <span className="text-sm font-semibold text-text-primary">Health Timeline</span>
            <HealthTimeline hours={24} interval={30} />
          </div>
          <div className="mt-4 flex flex-col gap-2">
            <span className="text-sm font-semibold text-text-primary">Service Details</span>
            <div className="rounded border text-sm" style={{ borderColor: 'var(--border-primary)' }}>
              {[
                ['Service ID', 'svc_a1b2c3d4', true],
                ['Environment', 'production'],
                ['Host', 'api-01, api-02, api-03'],
                ['Version', 'v2.14.1'],
                ['Uptime', '99.8%'],
                ['Last deployed', '2h ago'],
                ['First seen', '2026-01-15'],
              ].map(([label, value, copyable], i) => (
                <div
                  key={label as string}
                  className="flex items-center px-4 py-2"
                  style={{ borderBottom: i < 6 ? '1px solid var(--border-primary)' : 'none' }}
                >
                  <span className="w-36 shrink-0" style={{ color: 'var(--text-secondary)' }}>{label as string}</span>
                  <span className="text-text-primary font-mono">{value as string}</span>
                  {copyable && (
                    <button
                      className="ml-2 text-xs px-1.5 py-0.5 rounded hover:bg-[var(--hover-bg)] transition-colors"
                      style={{ color: 'var(--accent)' }}
                      onClick={() => navigator.clipboard.writeText(value as string)}
                    >
                      Copy
                    </button>
                  )}
                </div>
              ))}
            </div>
          </div>
          <div className="mt-4 flex flex-col gap-2">
            <span className="text-sm font-semibold text-text-primary">Recent Errors (5)</span>
            <div className="rounded border text-sm font-mono overflow-hidden" style={{ borderColor: 'var(--border-primary)' }}>
              <div className="flex items-center h-9 border-b text-sm font-medium" style={{ borderColor: 'var(--border-primary)', backgroundColor: 'color-mix(in srgb, var(--bg-primary) 40%, var(--bg-secondary))' }}>
                <div className="w-[60px] shrink-0 h-full border-r border-l flex items-center justify-center" style={{ borderColor: 'var(--border-primary)' }}>
                  <span className="text-xs" style={{ color: 'var(--text-secondary)' }}>View</span>
                </div>
                <div className="w-[180px] shrink-0 px-4 h-full border-r flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>timestamp</div>
                <div className="w-[90px] shrink-0 px-4 h-full border-r flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>level</div>
                <div className="w-[160px] shrink-0 px-4 h-full border-r flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>error_code</div>
                <div className="w-[400px] shrink-0 px-4 h-full border-r flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>message</div>
                <div className="w-[100px] shrink-0 px-4 h-full border-r flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>latency</div>
                <div className="w-[90px] shrink-0 px-4 h-full flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>freq</div>
              </div>
              {[
                { time: '14:23:11', level: 'error', code: '503', message: 'Service unavailable', latency: '45ms', freq: '12x' },
                { time: '14:22:05', level: 'error', code: '502', message: 'Bad gateway', latency: '120ms', freq: '8x' },
                { time: '14:20:33', level: 'error', code: '408', message: 'Request timeout', latency: '30s', freq: '5x' },
                { time: '14:18:12', level: 'critical', code: '500', message: 'Internal error', latency: '250ms', freq: '3x' },
                { time: '14:15:44', level: 'error', code: '429', message: 'Rate limit', latency: '0ms', freq: '45x' },
              ].map((err, i) => (
                <button
                  key={i}
                  className={`flex items-center w-full text-left hover:bg-[var(--hover-bg)] transition-colors cursor-pointer py-1 ${i < 4 ? 'border-b' : ''}`}
                  style={{ borderColor: 'var(--border-primary)' }}
                  onClick={() => navigate(`/errors?service=${service?.service}`)}
                >
                  <div className="w-[60px] shrink-0 h-full border-r border-l flex items-center justify-center" style={{ borderColor: 'var(--border-primary)' }}>
                    <span className="text-sm" style={{ color: 'var(--accent)' }}>View</span>
                  </div>
                  <div className="w-[180px] shrink-0 px-3 truncate border-r h-full flex items-center" style={{ color: 'var(--text-secondary)', borderColor: 'var(--border-primary)' }}>{err.time}</div>
                  <div className="w-[90px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center" style={{ color: 'var(--error)', borderColor: 'var(--border-primary)' }}>{err.level}</div>
                  <div className="w-[160px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center" style={{ color: 'var(--error)', borderColor: 'var(--border-primary)' }}>{err.code}</div>
                  <div className="w-[400px] shrink-0 px-3 text-text-primary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{err.message}</div>
                  <div className="w-[100px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{err.latency}</div>
                  <div className="w-[90px] shrink-0 px-3 text-text-primary truncate h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{err.freq}</div>
                </button>
              ))}
            </div>
          </div>
          <div className="mt-4 flex flex-col gap-2">
            <span className="text-sm font-semibold text-text-primary">Related Logs (last 5)</span>
            <div className="rounded border text-sm font-mono overflow-hidden" style={{ borderColor: 'var(--border-primary)' }}>
              <div className="flex items-center h-9 border-b text-sm font-medium" style={{ borderColor: 'var(--border-primary)', backgroundColor: 'color-mix(in srgb, var(--bg-primary) 40%, var(--bg-secondary))' }}>
                <div className="w-[60px] shrink-0 h-full border-r border-l flex items-center justify-center" style={{ borderColor: 'var(--border-primary)' }}>
                  <span className="text-xs" style={{ color: 'var(--text-secondary)' }}>View</span>
                </div>
                <div className="w-[180px] shrink-0 px-4 h-full border-r flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>timestamp</div>
                <div className="w-[100px] shrink-0 px-4 h-full border-r flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>level</div>
                <div className="w-[130px] shrink-0 px-4 h-full border-r flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>status_code</div>
                <div className="w-[400px] shrink-0 px-4 h-full border-r flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>message</div>
                <div className="w-[130px] shrink-0 px-4 h-full flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>response</div>
              </div>
              {[
                { time: '14:23:11', level: 'error', code: '503', message: 'Connection pool exhausted', response: '45ms' },
                { time: '14:22:05', level: 'error', code: '502', message: 'Upstream refused connection', response: '2.1s' },
                { time: '14:20:33', level: 'warn', code: '200', message: 'Slow query detected (2.3s)', response: '2.3s' },
                { time: '14:18:12', level: 'error', code: '500', message: 'Handler crashed on request', response: '150ms' },
                { time: '14:15:44', level: 'info', code: '200', message: 'Health check passed', response: '12ms' },
              ].map((log, i) => {
                const levelColor = log.level === 'error' ? 'var(--error)' : log.level === 'warn' ? 'var(--warn)' : 'var(--info)'
                const statusColor = parseInt(log.code) >= 500 ? 'var(--error)' : parseInt(log.code) >= 400 ? 'var(--warn)' : 'var(--success)'
                return (
                  <button
                    key={i}
                    className={`flex items-center w-full text-left hover:bg-[var(--hover-bg)] transition-colors cursor-pointer py-1 ${i < 4 ? 'border-b' : ''}`}
                    style={{ borderColor: 'var(--border-primary)' }}
                    onClick={() => navigate(`/logs?service=${service?.service}`)}
                  >
                    <div className="w-[60px] shrink-0 h-full border-r border-l flex items-center justify-center" style={{ borderColor: 'var(--border-primary)' }}>
                      <span className="text-sm" style={{ color: 'var(--accent)' }}>View</span>
                    </div>
                    <div className="w-[180px] shrink-0 px-3 truncate border-r h-full flex items-center" style={{ color: 'var(--text-secondary)', borderColor: 'var(--border-primary)' }}>{log.time}</div>
                    <div className="w-[100px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center" style={{ color: levelColor, borderColor: 'var(--border-primary)' }}>{log.level}</div>
                    <div className="w-[130px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center" style={{ color: statusColor, borderColor: 'var(--border-primary)' }}>{log.code}</div>
                    <div className="w-[400px] shrink-0 px-3 text-text-primary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{log.message}</div>
                    <div className="w-[130px] shrink-0 px-3 text-text-secondary truncate h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{log.response}</div>
                  </button>
                )
              })}
            </div>
          </div>
          <div className="mt-4 flex items-center gap-1">
            <button
              className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
              style={{ color: 'var(--accent)' }}
              onClick={() => navigate(`/logs?service=${service?.service}`)}
            >
              View in Logs
            </button>
            <button
              className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
              style={{ color: 'var(--accent)' }}
              onClick={() => navigate(`/errors?service=${service?.service}`)}
            >
              View in Errors
            </button>
            <button
              className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
              style={{ color: 'var(--accent)' }}
              onClick={() => navigate(`/analytics?service=${service?.service}`)}
            >
              View in Analytics
            </button>
          </div>
        </div>
      </div>
    </>
  )
}
