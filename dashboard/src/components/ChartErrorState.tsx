import { LuAlertCircle, LuRefreshCw } from 'react-icons/lu'

interface ChartErrorStateProps {
  message?: string
  onRetry?: () => void
}

export default function ChartErrorState({ message = 'An error occurred. Please try again.', onRetry }: ChartErrorStateProps) {
  return (
    <div className="flex flex-col items-center justify-center h-full gap-3 px-4">
      <LuAlertCircle className="size-12 shrink-0" style={{ color: 'var(--error)' }} />
      <p className="text-sm text-center" style={{ color: 'var(--text-secondary)' }}>
        {message}
      </p>
      {onRetry && (
        <button
          onClick={onRetry}
          className="flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded border transition-all hover:opacity-80"
          style={{
            color: 'var(--error)',
            borderColor: 'var(--error)',
            backgroundColor: 'transparent',
          }}
        >
          <LuRefreshCw className="size-3.5" />
          <span>Try again</span>
        </button>
      )}
    </div>
  )
}
