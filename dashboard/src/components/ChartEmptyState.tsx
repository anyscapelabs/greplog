export default function ChartEmptyState({ message = 'No live data — chart not yet connected' }: { message?: string }) {
  return (
    <div className="flex items-center justify-center h-full w-full">
      <span className="text-sm" style={{ color: 'var(--text-secondary)' }}>{message}</span>
    </div>
  )
}
