import { useState } from 'react'
import { LuX, LuCopy, LuChevronDown, LuChevronRight } from 'react-icons/lu'
import { useNavigate } from 'react-router-dom'
import Editor from '@monaco-editor/react'

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
          <span className="text-sm font-semibold text-text-primary flex items-center gap-2">Error Details<LuChevronRight className="size-3" style={{ color: 'var(--text-secondary)' }} /><span style={{ color: 'var(--accent)' }}>err_{error?.id}</span>
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
          <button
            className="flex items-center justify-center size-7 rounded hover:bg-[var(--hover-bg)] transition-colors"
            onClick={onClose}
          >
            <LuX className="size-4" style={{ color: 'var(--text-secondary)' }} />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-4">
          {error && (
            <>
              <div className="flex flex-col gap-1.5">
                <span className="text-sm font-semibold text-text-primary">Metadata</span>
                <div className="rounded border text-sm" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
                  {[
                    ['service_name', error.service],
                    ['event_id', `err_${error.id}`, 'copyable'],
                    ['severity', error.level, 'level'],
                    ['timestamp', error.timestamp],
                    ['correlation_id', `corr-${error.id}a9f`, 'copyable'],
                  ].map(([label, value, type], i) => (
                    <div
                      key={label as string}
                      className="flex items-center px-4 py-2"
                      style={{ borderBottom: i < 4 ? '1px solid var(--border-primary)' : 'none' }}
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
              <div className="flex flex-col gap-1.5">
                <span className="text-sm font-semibold text-text-primary">Exception Details</span>
                <div className="rounded border text-sm" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
                  {[
                    ['exception_type', 'Error'],
                    ['exception_message', error.message],
                    ['http.status_code', String(error.errorCode)],
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
              </div>
              <div className="rounded border text-sm" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
                {[
                  ['Frequency', `${error.freq}`],
                  ['First seen', '2026-07-17 12:01'],
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
              <div className="flex flex-col gap-1.5">
                <button
                  className="flex items-center gap-1.5 text-sm font-semibold text-text-primary cursor-pointer hover:opacity-80 transition-opacity"
                  onClick={() => setTraceOpen(!traceOpen)}
                >
                  <LuChevronDown className={`size-3 transition-transform ${traceOpen ? '' : '-rotate-90'}`} style={{ color: 'var(--text-secondary)' }} />
                  Stack Trace
                </button>
                {traceOpen && (
                  <div className="rounded border overflow-hidden h-32" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
                    <Editor
                      height="100%"
                      defaultLanguage="text"
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
                      value={`Error: ${error.message}
    at src/server/handler.ts:142
    at src/middleware/rate.ts:89
    at src/middleware/auth.ts:45
    at src/server/index.ts:210`}
                    />
                  </div>
                )}
              </div>
              <div className="flex flex-col gap-1.5">
                <span className="text-sm font-semibold text-text-primary">Affected Endpoints</span>
                <div className="rounded border font-mono text-sm overflow-hidden" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
                  <div className="flex items-center h-9 border-b font-medium" style={{ borderColor: 'var(--border-primary)', backgroundColor: 'color-mix(in srgb, var(--bg-primary) 40%, var(--bg-secondary))' }}>
                    <div className="w-[80px] shrink-0 px-4 h-full border-r flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>method</div>
                    <div className="flex-1 shrink-0 px-4 h-full border-r flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>endpoint</div>
                    <div className="w-[100px] shrink-0 px-4 h-full flex items-center text-text-primary" style={{ borderColor: 'var(--border-primary)' }}>count</div>
                  </div>
                  {[
                    { method: 'POST', endpoint: '/api/events', count: '45x' },
                    { method: 'GET', endpoint: '/api/stream', count: '2x' },
                    { method: 'PUT', endpoint: '/api/config', count: '1x' },
                  ].map((ep, i) => (
                    <div
                      key={i}
                      className="flex items-center w-full py-1"
                      style={{ borderBottom: i < 2 ? '1px solid var(--border-primary)' : 'none', borderColor: 'var(--border-primary)' }}
                    >
                      <div className="w-[80px] shrink-0 px-3 truncate border-r h-full flex items-center font-medium" style={{ color: 'var(--accent)', borderColor: 'var(--border-primary)' }}>{ep.method}</div>
                      <div className="flex-1 shrink-0 px-3 truncate border-r h-full flex items-center" style={{ color: 'var(--text-primary)', borderColor: 'var(--border-primary)' }}>{ep.endpoint}</div>
                      <div className="w-[100px] shrink-0 px-3 truncate h-full flex items-center" style={{ color: 'var(--text-secondary)' }}>{ep.count}</div>
                    </div>
                  ))}
                </div>
              </div>
            </>
          )}
        </div>
        {error && (
          <div className="flex items-center gap-2 px-4 py-3 border-t shrink-0 flex-wrap" style={{ borderColor: 'var(--border-primary)', backgroundColor: 'var(--bg-secondary)' }}>
            <button
              className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
              style={{ color: 'var(--accent)' }}
              onClick={() => navigator.clipboard.writeText(`err_${error.id}`)}
            >
              Copy ID
            </button>
            <button
              className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
              style={{ color: 'var(--accent)' }}
              onClick={() => navigate(`/logs?service=${error.service}`)}
            >
              View Related Logs
            </button>
          </div>
        )}
      </div>
    </>
  )
}
