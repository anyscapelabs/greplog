import { useState } from 'react'
import { LuCopy, LuChevronDown } from 'react-icons/lu'
import { useNavigate } from 'react-router-dom'
import Editor from '@monaco-editor/react'
import Drawer from './Drawer.tsx'

interface LogsDrawerProps {
  open: boolean
  onClose: () => void
  log: any
}

export default function LogsDrawer({ open, onClose, log }: LogsDrawerProps) {
  const navigate = useNavigate()
  const [traceOpen, setTraceOpen] = useState(true)
  const [payloadOpen, setPayloadOpen] = useState(false)
  if (!open) return null

  const rawLogPayload = log ? JSON.stringify({
    id: log.id,
    timestamp: log.timestamp,
    level: log.level,
    message: log.message,
    service_name: log.service,
    logger_name: log.logger || undefined,
    correlation_id: log.correlationId || undefined,
    file: log.file || undefined,
  }, null, 2) : ''

  return (
    <Drawer
      open={open}
      onClose={onClose}
      width="1200px"
      title={
        <span className="flex items-center gap-2">
          Log Details
          <span style={{ color: 'var(--accent)' }}>{log?.id}</span>
          {log && (
            <>
              <span
                className="px-2.5 py-0.5 rounded-full text-xs font-semibold capitalize"
                style={{
                  backgroundColor: log.level === 'error' ? 'color-mix(in srgb, var(--error) 20%, transparent)' : log.level === 'warn' ? 'color-mix(in srgb, var(--warn) 20%, transparent)' : log.level === 'info' ? 'color-mix(in srgb, var(--info) 20%, transparent)' : 'color-mix(in srgb, var(--text-secondary) 20%, transparent)',
                  color: log.level === 'error' ? 'var(--error)' : log.level === 'warn' ? 'var(--warn)' : log.level === 'info' ? 'var(--info)' : 'var(--text-secondary)',
                }}
              >
                {log.level}
              </span>
              <span className="text-xs" style={{ color: 'var(--text-secondary)' }}>{log.timestamp}</span>
            </>
          )}
        </span>
      }
    >
      {log && (
        <>
          <div className="flex flex-col gap-1.5">
            <span className="text-sm font-semibold text-text-primary">Message</span>
            <div className="rounded border relative" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              <button
                className="absolute top-1.5 right-1.5 flex items-center justify-center size-7 rounded hover:bg-[var(--hover-bg)] transition-colors"
                onClick={() => navigator.clipboard.writeText(log.message)}
              >
                <LuCopy className="size-3.5" style={{ color: 'var(--text-secondary)' }} />
              </button>
              <div className="p-3 pr-10 text-sm font-mono whitespace-pre-wrap break-words" style={{ color: 'var(--text-primary)' }}>
                {log.message}
              </div>
            </div>
          </div>
          <div className="flex flex-col gap-1.5 mt-4">
            <span className="text-sm font-semibold text-text-primary">Metadata</span>
            <div className="rounded border text-sm" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              {[
                ['service_name', log.service, 'service'],
                ['event_id', log.id, 'copyable'],
                ['correlation_id', log.correlationId || '—', log.correlationId ? 'correlation_id' : undefined],
                ['logger_name', log.logger || '—'],
                ['file', log.file || '—'],
              ].map(([label, value, metaType], i) => {
                const isLast = i === 4
                return (
                  <div
                    key={label as string}
                    className="flex items-center px-4 py-2"
                    style={{ borderBottom: isLast ? 'none' : '1px solid var(--border-primary)' }}
                  >
                    <span className="w-48 shrink-0 text-xs font-mono" style={{ color: 'var(--text-secondary)' }}>{label as string}</span>
                    {metaType === 'service' ? (
                      <button
                        className="font-mono text-xs hover:underline cursor-pointer"
                        style={{ color: 'var(--accent)' }}
                        onClick={() => navigate(`/logs?c=service:${value}`)}
                      >
                        {value as string}
                      </button>
                    ) : metaType === 'correlation_id' ? (
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
                    ) : metaType === 'copyable' ? (
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
                )
              })}
            </div>
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
                <span style={{ color: log.stackTrace ? 'var(--text-primary)' : 'var(--text-secondary)' }}>
                  {log.stackTrace || 'No stack trace recorded for this event.'}
                </span>
              </div>
            )}
          </div>
          <div className="flex flex-col gap-1.5 mt-4">
            <button
              className="flex items-center gap-1.5 text-sm font-semibold text-text-primary cursor-pointer hover:opacity-80 transition-opacity"
              onClick={() => setPayloadOpen(!payloadOpen)}
            >
              <LuChevronDown className={`size-3 transition-transform ${payloadOpen ? '' : '-rotate-90'}`} style={{ color: 'var(--text-secondary)' }} />
              Raw Log Payload
            </button>
            {payloadOpen && (
              <div className="rounded border overflow-hidden h-64" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
                <Editor
                  height="100%"
                  defaultLanguage="json"
                  theme="vs-dark"
                  options={{
                    minimap: { enabled: false },
                    readOnly: true,
                    wordWrap: 'on',
                    scrollBeyondLastLine: false,
                    padding: { top: 12, bottom: 12 },
                    lineNumbers: 'off',
                    renderLineHighlight: 'none',
                    folding: false,
                  }}
                  value={rawLogPayload}
                />
              </div>
            )}
          </div>
          <div className="flex flex-col gap-1.5 mt-4">
            <span className="text-sm font-semibold text-text-primary">Related Errors</span>
            <div className="rounded border font-mono text-sm overflow-hidden" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
              <div className="flex items-center h-9 border-b font-medium" style={{ borderColor: 'var(--border-primary)', backgroundColor: 'color-mix(in srgb, var(--bg-primary) 40%, var(--bg-secondary))' }}>
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
              {log.correlationId ? (
                <div className="p-4 text-sm" style={{ color: 'var(--text-secondary)' }}>
                  Query errors for{' '}
                  <button
                    className="hover:underline cursor-pointer"
                    style={{ color: 'var(--accent)' }}
                    onClick={() => navigate(`/errors?c=correlation_id:${log.correlationId}`)}
                  >
                    correlation_id: {log.correlationId}
                  </button>
                  {' '}to see related entries.
                </div>
              ) : (
                <div className="p-4 text-sm" style={{ color: 'var(--text-secondary)' }}>
                  No correlation_id available on this log entry.
                </div>
              )}
            </div>
          </div>
          <div className="flex items-center gap-2 mt-4 pt-3 border-t flex-wrap" style={{ borderColor: 'var(--border-primary)' }}>
            <button
              className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
              style={{ color: 'var(--accent)' }}
              onClick={() => navigator.clipboard.writeText(String(log.id))}
            >
              Copy ID
            </button>
            <button
              className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
              style={{ color: 'var(--accent)' }}
              onClick={() => navigator.clipboard.writeText(rawLogPayload)}
            >
              Copy JSON
            </button>
            <button
              className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
              style={{ color: 'var(--accent)' }}
              onClick={() => navigate(`/errors?c=correlation_id:${log.correlationId}`)}
            >
              View Related Errors →
            </button>
            <button
              className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
              style={{ color: 'var(--accent)' }}
              onClick={() => navigate(`/logs?c=service:${log.service}`)}
            >
              View in Logs
            </button>
          </div>
        </>
      )}
    </Drawer>
  )
}
