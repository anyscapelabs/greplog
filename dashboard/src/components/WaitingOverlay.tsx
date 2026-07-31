import { LuTerminal } from 'react-icons/lu'

export const SDK_SETUP_TERMINAL: string[] = [
  '# Node — auto-instrumenting',
  'npm install greplog',
  '',
  'import greplog from \'greplog\'',
  'greplog.init()',
  '',
  '# Python — auto-instrumenting',
  'pip install greplog',
  '',
  'import greplog',
  'greplog.init()',
  '',
  '# Go',
  'go get github.com/greplog/greplog-go',
  '',
  'import "github.com/greplog/greplog-go"',
  '',
  'greplog.Init()',
  '',
  '# Rust',
  'cargo add greplog',
  '',
  'greplog::init();',
]

interface WaitingOverlayProps {
  message?: string
  visible: boolean
  terminal?: string[]
}

export default function WaitingOverlay({ message = 'Waiting for data...', visible, terminal }: WaitingOverlayProps) {
  if (!visible) return null

  return (
    <div className="absolute inset-0 z-20 flex items-center justify-center pointer-events-auto">
      <div
        className="absolute inset-0 backdrop-blur-sm"
        style={{ backgroundColor: 'rgba(0, 0, 0, 0.03)' }}
      />
      <div className="relative z-10 flex flex-col items-center gap-4 px-8 py-8 rounded-lg min-w-[480px]" style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)', borderWidth: 1 }}>
        <LuTerminal className="size-10" style={{ color: 'var(--text-secondary)' }} />
        <span className="text-lg font-medium text-center" style={{ color: 'var(--text-primary)' }}>{message}</span>
        {terminal && (
          <div
            className="w-full rounded-lg p-4 font-mono text-sm leading-relaxed"
            style={{ backgroundColor: '#0d1117', color: '#e6edf3' }}
          >
            {terminal.map((line, i) => {
              const isPrompt = line.startsWith('$ ')
              const isComment = line.startsWith('#')
              return (
                <div key={i} className="whitespace-pre-wrap">
                  {isPrompt ? (
                    <><span style={{ color: '#3fb950' }}>$</span> {line.slice(2)}</>
                  ) : isComment ? (
                    <span style={{ color: '#8b949e' }}>{line}</span>
                  ) : (
                    <span>{line}</span>
                  )}
                </div>
              )
            })}
          </div>
        )}
      </div>
    </div>
  )
}