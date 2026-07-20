import { useState } from 'react'
import { LuX, LuChevronRight, LuCopy, LuChevronDown } from 'react-icons/lu'
import { useNavigate } from 'react-router-dom'

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
          <span className="text-sm font-semibold text-text-primary flex items-center gap-2">Log Details<LuChevronRight className="size-3" style={{ color: 'var(--text-secondary)' }} /><span style={{ color: 'var(--accent)' }}>{log?.id}</span>
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
          <button
            className="flex items-center justify-center size-7 rounded hover:bg-[var(--hover-bg)] transition-colors"
            onClick={onClose}
          >
            <LuX className="size-4" style={{ color: 'var(--text-secondary)' }} />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-4">
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
              <div className="flex flex-col gap-1.5">
                <span className="text-sm font-semibold text-text-primary">Context</span>
                <div className="rounded border text-sm" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
                  {[
                    ['Service', log.service],
                    ['Status', log.statusCode],
                    ['Response', log.response],
                    ['Logger', log.logger],
                    ['File', log.file, true],
                    ['Correlation', log.correlationId, true],
                    ['Environment', 'production'],
                    ['Host', 'db-03.internal'],
                  ].map(([label, value, isSpecial], i) => (
                    <div
                      key={label as string}
                      className="flex items-center px-4 py-2"
                      style={{ borderBottom: i < 7 ? '1px solid var(--border-primary)' : 'none' }}
                    >
                      <span className="w-28 shrink-0 text-xs" style={{ color: 'var(--text-secondary)' }}>{(label as string)}</span>
                      {isSpecial && label === 'File' ? (
                        <button
                          className="font-mono text-xs hover:underline cursor-pointer"
                          style={{ color: 'var(--accent)' }}
                          onClick={() => {/* open in editor */}}
                        >
                          {value as string}
                        </button>
                      ) : isSpecial && label === 'Correlation' ? (
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
                <button
                  className="flex items-center gap-1.5 text-sm font-semibold text-text-primary cursor-pointer hover:opacity-80 transition-opacity"
                  onClick={() => setTraceOpen(!traceOpen)}
                >
                  <LuChevronDown className={`size-3 transition-transform ${traceOpen ? '' : '-rotate-90'}`} style={{ color: 'var(--text-secondary)' }} />
                  Stack Trace
                </button>
                {traceOpen && (
                  <div className="rounded border font-mono text-xs" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
                    <div className="p-3 leading-relaxed whitespace-pre" style={{ color: 'var(--text-primary)' }}>
                      {`Error: ConnectionTimeout
    at Pool.acquire (src/db/pool.ts:142)
    at QueryRunner.run (src/db/query.ts:89)
    at RequestHandler.exec (src/http/handler.ts:201)
    at Server.process (src/server.ts:56)`}
                    </div>
                  </div>
                )}
              </div>
              <div className="flex flex-col gap-1.5">
                <button
                  className="flex items-center gap-1.5 text-sm font-semibold text-text-primary cursor-pointer hover:opacity-80 transition-opacity"
                  onClick={() => setPayloadOpen(!payloadOpen)}
                >
                  <LuChevronDown className={`size-3 transition-transform ${payloadOpen ? '' : '-rotate-90'}`} style={{ color: 'var(--text-secondary)' }} />
                  Raw Log Payload
                </button>
                {payloadOpen && (
                  <div className="rounded border font-mono text-xs overflow-x-auto" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}>
                    <div className="p-3 leading-relaxed whitespace-pre" style={{ color: 'var(--text-primary)' }}>
                      {`{
  "level": "${log.level}",
  "message": "${log.message}",
  "service": "${log.service}",
  "status_code": ${log.statusCode},
  "response_time_ms": "${log.response}",
  "logger": "${log.logger}",
  "correlation_id": "${log.correlationId}",
  "file": "${log.file}"
}`}
                    </div>
                  </div>
                )}
              </div>
              <div className="flex flex-col gap-1.5">
                <span className="text-sm font-semibold text-text-primary">Related Errors (3)</span>
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
                  {[
                    { time: '14:23:11', level: 'error', code: '503', message: 'Service unavailable', latency: '45ms', freq: '12x' },
                    { time: '14:22:05', level: 'error', code: '502', message: 'Bad gateway', latency: '120ms', freq: '8x' },
                    { time: '14:20:33', level: 'critical', code: '500', message: 'Handler crash', latency: '250ms', freq: '3x' },
                  ].map((err, i) => (
                    <button
                      key={i}
                      className={`flex items-center w-full text-left hover:bg-[var(--hover-bg)] transition-colors cursor-pointer py-1 ${i < 2 ? 'border-b' : ''}`}
                      style={{ borderColor: 'var(--border-primary)' }}
                      onClick={() => navigate(`/errors?correlationId=${log.correlationId}`)}
                    >
                      <div className="w-[60px] shrink-0 h-full border-r border-l flex items-center justify-center" style={{ borderColor: 'var(--border-primary)' }}>
                        <span className="text-sm" style={{ color: 'var(--accent)' }}>View</span>
                      </div>
                      <div className="w-[180px] shrink-0 px-3 truncate border-r h-full flex items-center" style={{ color: 'var(--text-secondary)', borderColor: 'var(--border-primary)' }}>{err.time}</div>
                      <div className="w-[90px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center" style={{ color: err.level === 'critical' || err.level === 'error' ? 'var(--error)' : 'var(--warn)', borderColor: 'var(--border-primary)' }}>{err.level}</div>
                      <div className="w-[160px] shrink-0 px-3 font-medium truncate border-r h-full flex items-center" style={{ color: 'var(--error)', borderColor: 'var(--border-primary)' }}>{err.code}</div>
                      <div className="w-[400px] shrink-0 px-3 truncate border-r h-full flex items-center" style={{ color: 'var(--text-primary)', borderColor: 'var(--border-primary)' }}>{err.message}</div>
                      <div className="w-[100px] shrink-0 px-3 truncate border-r h-full flex items-center" style={{ color: 'var(--text-secondary)', borderColor: 'var(--border-primary)' }}>{err.latency}</div>
                      <div className="w-[90px] shrink-0 px-3 truncate h-full flex items-center" style={{ color: 'var(--text-primary)', borderColor: 'var(--border-primary)' }}>{err.freq}</div>
                    </button>
                  ))}
                </div>
              </div>
              <div className="flex items-center gap-2 pt-2 border-t flex-wrap" style={{ borderColor: 'var(--border-primary)' }}>
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
                  onClick={() => {
                    const json = JSON.stringify({
                      level: log.level,
                      message: log.message,
                      service: log.service,
                      status_code: log.statusCode,
                      response_time_ms: log.response,
                      logger: log.logger,
                      correlation_id: log.correlationId,
                      file: log.file,
                    }, null, 2)
                    navigator.clipboard.writeText(json)
                  }}
                >
                  Copy JSON
                </button>
                <button
                  className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
                  style={{ color: 'var(--accent)' }}
                  onClick={() => navigate(`/errors?correlationId=${log.correlationId}`)}
                >
                  View Related Errors →
                </button>
                <button
                  className="px-2.5 py-1.5 text-sm cursor-pointer transition-colors hover:underline"
                  style={{ color: 'var(--accent)' }}
                  onClick={() => navigate(`/logs?service=${log.service}`)}
                >
                  View in Logs
                </button>
              </div>
            </>
          )}
        </div>
      </div>
    </>
  )
}
