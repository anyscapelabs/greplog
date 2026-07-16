import type { ReactNode } from 'react'

interface ChartCardProps {
  title: string
  children: ReactNode
  controls?: ReactNode
  height?: string
}

export default function ChartCard({ title, children, controls, height = 'h-64' }: ChartCardProps) {
  return (
    <div
      className={`flex-1 ${height} rounded border flex flex-col`}
      style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}
    >
      <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
        <span className="text-sm font-semibold text-text-primary">{title}</span>
        {controls && <div className="flex items-center gap-2 ml-auto">{controls}</div>}
      </div>
      <div className="flex-1 p-1">
        {children}
      </div>
    </div>
  )
}
