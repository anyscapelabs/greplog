import { useMemo } from 'react'
import { useNavigate } from 'react-router-dom'
import Drawer from './Drawer.tsx'
import ServicesIcon from '../icons/ServicesIcon.tsx'
import { useChartTheme } from '../utils/useChartTheme.ts'
import HealthTimeline from './HealthTimeline.tsx'
import { useErrors, useLogs } from '../hooks/index.ts'
import type { ServiceEntry } from '../types/index.ts'

interface MetricDef {
  label: string
  value: string
  colorKey: string
}

interface ServicesDrawerProps {
  open: boolean
  onClose: () => void
  service: ServiceEntry | null
}

function formatStreamingSince(service: ServiceEntry): string {
  if (service.firstSeen) {
    const d = new Date(service.firstSeen)
    return `Streaming since ${d.toLocaleString()}`
  }
  return 'Not available'
}

function ServicesDrawerContent({ service, onClose }: { service: ServiceEntry; onClose: () => void }) {
  const navigate = useNavigate()
  const colors = useChartTheme()

  const safeName = service.name.replace(/'/g, "''")
  const whereClause = `service = '${safeName}'`
  const { errors } = useErrors(whereClause)
  const { logs } = useLogs(`WHERE ${whereClause}`)

  const recentErrors = useMemo(() => errors.slice(0, 5), [errors])
  const recentLogs = useMemo(() => logs.slice(0, 5), [logs])

  const metrics: MetricDef[] = useMemo(() => {
    const err = service.errorRate
    const pct = (err * 100).toFixed(1)
    const ec = service.eventCount
    return [
      { label: 'Error Rate', value: `${pct}%`, colorKey: err > 0.05 ? 'red' : err > 0.01 ? 'orange' : 'blue' },
      { label: 'Events', value: ec >= 1000 ? `${(ec / 1000).toFixed(1)}k` : String(ec), colorKey: 'blue' },
      { label: 'Health', value: service.health, colorKey: service.health === 'healthy' ? 'green' : service.health === 'degraded' ? 'orange' : service.health === 'unhealthy' ? 'red' : 'blue' },
    ]
  }, [service])

  return (
    <Drawer
      open
      onClose={onClose}
      width="1200px"
      title={
        <span className="flex items-center gap-2">
          <ServicesIcon className="size-4" style={{ color: 'var(--text-secondary)' }} />
          Service: {service.name}
        </span>
      }
    >
      <div className="flex gap-3 mb-6">
        {metrics.map((m, i) => {
          const colorKey = m.colorKey as keyof typeof colors
          const color = colors[colorKey]
          return (
            <div
              key={i}
              className="flex-1 rounded border h-[120px] relative overflow-hidden flex flex-col p-3"
              style={{ backgroundColor: `${color}15`, borderColor: 'var(--border-primary)' }}
            >
              <div className="z-10 flex flex-col relative">
                <span className="text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>{m.label}</span>
                <span className="text-xl font-bold mt-0.5 tracking-tight" style={{ color: 'var(--text-primary)' }}>{m.value}</span>
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
          {([
            ['Service ID', service.id, true],
            ['Environment', service.environment ?? 'production'],
            ['Host', 'Not available \u2014 requires SDK handshake'],
            ['Version', 'Not available \u2014 requires SDK handshake'],
            ['Streaming', formatStreamingSince(service)],
            ['Last deployed', service.lastDeployed ?? 'Not available'],
            ['First seen', service.firstSeen ? new Date(service.firstSeen).toLocaleString() : 'Not available'],
          ] as [string, string, boolean?][]).map(([label, value, copyable], i) => (
            <div
              key={label}
              className="flex items-center px-4 py-2"
              style={{ borderBottom: i < 6 ? '1px solid var(--border-primary)' : 'none' }}
            >
              <span className="w-36 shrink-0" style={{ color: 'var(--text-secondary)' }}>{label}</span>
              <span className="text-text-primary font-mono">{value}</span>
              {copyable && (
                <button
                  className="ml-2 text-xs px-1.5 py-0.5 rounded hover:bg-[var(--hover-bg)] transition-colors"
                  style={{ color: 'var(--accent)' }}
                  onClick={() => navigator.clipboard.writeText(value)}
                >
                  Copy
                </button>
              )}
            </div>
          ))}
        </div>
      </div>
      <div className="mt-4 flex flex-col gap-2">
        <span className="text-sm font-semibold text-text-primary">Recent Errors ({recentErrors.length})</span>
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
          {recentErrors.length === 0 ? (
            <div className="flex items-center justify-center h-16 text-sm" style={{ color: 'var(--text-secondary)' }}>
              No errors for this service
            </div>
          ) : (
            recentErrors.map((err, i) => (
              <button
                key={err.id}
                className={`flex items-center w-full text-left hover:bg-[var(--hover-bg)] transition-colors cursor-pointer py-1 ${i < recentErrors.length - 1 ? 'border-b' : ''}`}
                style={{ borderColor: 'var(--border-primary)' }}
                onClick={() => navigate(`/errors?c=service:${service.name}`)}
              >
                <div className="w-[60px] shrink-0 h-full border-r border-l flex items-center justify-center" style={{ borderColor: 'var(--border-primary)' }}>
                  <span className="text-sm" style={{ color: 'var(--accent)' }}>View</span>
                </div>
                <div className="w-[180px] shrink-0 px-3 truncate border-r h-full flex items-center" style={{ color: 'var(--text-secondary)', borderColor: 'var(--border-primary)' }}>
                  {new Date(err.timestamp).toLocaleTimeString()}
                </div>
                <div className="w-[90px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center" style={{ color: 'var(--error)', borderColor: 'var(--border-primary)' }}>{err.level}</div>
                <div className="w-[160px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center" style={{ color: 'var(--error)', borderColor: 'var(--border-primary)' }}>{err.errorCode}</div>
                <div className="w-[400px] shrink-0 px-3 text-text-primary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{err.message}</div>
                <div className="w-[100px] shrink-0 px-3 text-text-secondary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{err.latency || '\u2014'}</div>
                <div className="w-[90px] shrink-0 px-3 text-text-primary truncate h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{err.freq}</div>
              </button>
            ))
          )}
        </div>
      </div>
      <div className="mt-4 flex flex-col gap-2">
        <span className="text-sm font-semibold text-text-primary">Related Logs (last {recentLogs.length})</span>
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
          {recentLogs.length === 0 ? (
            <div className="flex items-center justify-center h-16 text-sm" style={{ color: 'var(--text-secondary)' }}>
              No logs for this service
            </div>
          ) : (
            recentLogs.map((log, i) => {
              const levelColor = log.level === 'error' || log.level === 'critical' ? 'var(--error)' : log.level === 'warn' ? 'var(--warn)' : 'var(--info)'
              const statusColor = log.statusCode >= 500 ? 'var(--error)' : log.statusCode >= 400 ? 'var(--warn)' : 'var(--success)'
              return (
                <button
                  key={log.id}
                  className={`flex items-center w-full text-left hover:bg-[var(--hover-bg)] transition-colors cursor-pointer py-1 ${i < recentLogs.length - 1 ? 'border-b' : ''}`}
                  style={{ borderColor: 'var(--border-primary)' }}
                  onClick={() => navigate(`/logs?c=service:${service.name}`)}
                >
                  <div className="w-[60px] shrink-0 h-full border-r border-l flex items-center justify-center" style={{ borderColor: 'var(--border-primary)' }}>
                    <span className="text-sm" style={{ color: 'var(--accent)' }}>View</span>
                  </div>
                  <div className="w-[180px] shrink-0 px-3 truncate border-r h-full flex items-center" style={{ color: 'var(--text-secondary)', borderColor: 'var(--border-primary)' }}>
                    {new Date(log.timestamp).toLocaleTimeString()}
                  </div>
                  <div className="w-[100px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center" style={{ color: levelColor, borderColor: 'var(--border-primary)' }}>{log.level}</div>
                  <div className="w-[130px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center" style={{ color: statusColor, borderColor: 'var(--border-primary)' }}>{log.statusCode}</div>
                  <div className="w-[400px] shrink-0 px-3 text-text-primary truncate border-r h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{log.message}</div>
                  <div className="w-[130px] shrink-0 px-3 text-text-secondary truncate h-full flex items-center" style={{ borderColor: 'var(--border-primary)' }}>{log.response || '\u2014'}</div>
                </button>
              )
            })
          )}
        </div>
      </div>
      <div className="mt-4 flex items-center gap-1">
        <button
          className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
          style={{ color: 'var(--accent)' }}
          onClick={() => navigate(`/logs?c=service:${service.name}`)}
        >
          View in Logs
        </button>
        <button
          className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
          style={{ color: 'var(--accent)' }}
          onClick={() => navigate(`/errors?c=service:${service.name}`)}
        >
          View in Errors
        </button>
        <button
          className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
          style={{ color: 'var(--accent)' }}
          onClick={() => navigate(`/analytics?c=service:${service.name}`)}
        >
          View in Analytics
        </button>
      </div>
    </Drawer>
  )
}

export default function ServicesDrawer({ open, onClose, service }: ServicesDrawerProps) {
  if (!open || !service) return null
  return <ServicesDrawerContent service={service} onClose={onClose} />
}
