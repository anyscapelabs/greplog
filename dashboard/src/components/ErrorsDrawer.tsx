import { useState } from 'react'
import { LuChevronDown, LuCopy } from 'react-icons/lu'
import { useNavigate } from 'react-router-dom'
import Drawer from './Drawer.tsx'

interface ErrorsDrawerProps {
  open: boolean
  onClose: () => void
  error: any
}

export default function ErrorsDrawer({ open, onClose, error }: ErrorsDrawerProps) {
  const navigate = useNavigate()
  const [traceOpen, setTraceOpen] = useState(true)
  if (!open) return null

  return (
    <Drawer
      open={open}
      onClose={onClose}
      width="1200px"
      title={
        <span className="flex items-center gap-2">
          Error Details
          <span style={{ color: 'var(--accent)' }}>err_{error?.id}</span>
          {error && (
            <>
              <span
                className="px-2.5 py-0.5 rounded-full text-xs font-semibold capitalize"
                style={{
                  backgroundColor: error.level === 'critical' || error.level === 'error' ? 'color-mix(in srgb, var(--error) 20%, transparent)' : 'color-mix(in srgb, var(--warn) 20%, transparent)',
                  color: error.level === 'critical' || error.level === 'error' ? 'var(--error)' : 'var(--warn)',
                }}
              >
                {error.level}
              </span>
              <span className="text-xs" style={{ color: 'var(--text-secondary)' }}>{error.timestamp}</span>
            </>
          )}
        </span>
      }
    >
      {error && (
        <>
          <div className="flex flex-col gap-1.5">
            <span className="text-sm font-semibold text-text-primary">Metadata</span>
            <div className="rounded border text-sm" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              {[
                ['service_name', error.service, 'service'],
                ['event_id', error.id, 'copyable'],
                ['severity', error.level, 'level'],
                ['timestamp', error.timestamp],
                ['correlation_id', error.correlationId || '—', error.correlationId ? 'correlation_id' : undefined],
                ['error_type', error.errorType || '—'],
              ].map(([label, value, type], i) => (
                <div
                  key={label as string}
                  className="flex items-center px-4 py-2"
                  style={{ borderBottom: i < 5 ? '1px solid var(--border-primary)' : 'none' }}
                >
                  <span className="w-48 shrink-0 text-xs font-mono" style={{ color: 'var(--text-secondary)' }}>{label as string}</span>
                  {type === 'level' ? (
                    <span
                      className="px-2 py-0.5 rounded-full text-xs font-semibold"
                      style={{
                        backgroundColor: error.level === 'critical' || error.level === 'error' ? 'color-mix(in srgb, var(--error) 20%, transparent)' : 'color-mix(in srgb, var(--warn) 20%, transparent)',
                        color: error.level === 'critical' || error.level === 'error' ? 'var(--error)' : 'var(--warn)',
                      }}
                    >
                      {value as string}
                    </span>
                  ) : type === 'correlation_id' ? (
                    <button
                      className="font-mono text-xs hover:underline cursor-pointer flex items-center gap-1.5"
                      style={{ color: 'var(--accent)' }}
                      onClick={() => navigate(`/errors?c=correlation_id:${value}`)}
                    >
                      {value as string}
                      <button
                        className="hover:bg-[var(--hover-bg)] rounded p-0.5 transition-colors"
                        onClick={(e) => { e.stopPropagation(); navigator.clipboard.writeText(value as string) }}
                      >
                        <LuCopy className="size-3" style={{ color: 'var(--text-secondary)' }} />
                      </button>
                    </button>
                  ) : type === 'service' ? (
                    <button
                      className="font-mono text-xs hover:underline cursor-pointer"
                      style={{ color: 'var(--accent)' }}
                      onClick={() => navigate(`/logs?c=service:${value}`)}
                    >
                      {value as string}
                    </button>
                  ) : type === 'copyable' ? (
                    <span className="font-mono text-xs flex items-center gap-1.5" style={{ color: 'var(--text-primary)' }}>
                      {value as string}
                      <button
                        className="hover:bg-[var(--hover-bg)] rounded p-0.5 transition-colors"
                        onClick={() => navigator.clipboard.writeText(value as string)}
                      >
                        <LuCopy className="size-3" style={{ color: 'var(--text-secondary)' }} />
                      </button>
                    </span>
                  ) : (
                    <span className="font-mono text-xs" style={{ color: 'var(--text-primary)' }}>{value as string}</span>
                  )}
                </div>
              ))}
            </div>
          </div>
          <div className="flex flex-col gap-1.5 mt-4">
            <span className="text-sm font-semibold text-text-primary">Exception Details</span>
            <div className="rounded border text-sm" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              {[
                ['exception_message', error.message],
                ['error_code', String(error.errorCode)],
              ].map(([label, value], i) => (
                <div
                  key={label as string}
                  className="flex items-center px-4 py-2"
                  style={{ borderBottom: i < 1 ? '1px solid var(--border-primary)' : 'none' }}
                >
                  <span className="w-48 shrink-0 text-xs font-mono" style={{ color: 'var(--text-secondary)' }}>{label as string}</span>
                  <span className="font-mono text-xs" style={{ color: 'var(--text-primary)' }}>{value as string}</span>
                </div>
              ))}
            </div>
          </div>
          <div className="rounded border text-sm mt-4" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
            {[
              ['Frequency', error.freq ? `${error.freq}x` : '—'],
              ['First seen', error.firstSeen || '—'],
              ['Last seen', error.timestamp],
            ].map(([label, value], i) => (
              <div
                key={label as string}
                className="flex items-center px-4 py-2"
                style={{ borderBottom: i < 2 ? '1px solid var(--border-primary)' : 'none' }}
              >
                <span className="w-48 shrink-0 text-xs font-mono" style={{ color: 'var(--text-secondary)' }}>{label as string}</span>
                <span className="font-mono text-xs" style={{ color: 'var(--text-primary)' }}>{value as string}</span>
              </div>
            ))}
          </div>
          <div className="flex flex-col gap-1.5 mt-4">
            <button
              className="flex items-center gap-1.5 text-sm font-semibold text-text-primary cursor-pointer hover:opacity-80 transition-opacity"
              onClick={() => setTraceOpen(!traceOpen)}
            >
              <LuChevronDown className={`size-3 transition-transform ${traceOpen ? '' : '-rotate-90'}`} style={{ color: 'var(--text-secondary)' }} />
              Stack Trace
            </button>
            {traceOpen && (
              <div className="rounded border p-4 text-sm font-mono whitespace-pre-wrap break-words" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
                <span style={{ color: error.stackTrace ? 'var(--text-primary)' : 'var(--text-secondary)' }}>
                  {error.stackTrace || 'No stack trace recorded for this event.'}
                </span>
              </div>
            )}
          </div>
          <div className="flex items-center gap-2 mt-4 pt-3 border-t flex-wrap" style={{ borderColor: 'var(--border-primary)' }}>
            <button
              className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
              style={{ color: 'var(--accent)' }}
              onClick={() => navigator.clipboard.writeText(error.id)}
            >
              Copy ID
            </button>
            <button
              className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
              style={{ color: 'var(--accent)' }}
              onClick={() => navigate(`/logs?c=service:${error.service}`)}
            >
              View Related Logs
            </button>
          </div>
        </>
      )}
    </Drawer>
  )
}
