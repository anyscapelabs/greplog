import { useMemo } from 'react'

interface HealthTimelineProps {
  hours?: number
  interval?: 30 | 60 | 120
}

function generateTimeline(hours: number, interval: number) {
  const items: { time: string; status: string }[] = []
  const now = new Date()
  const steps = (hours * 60) / interval
  for (let i = steps - 1; i >= 0; i--) {
    const t = new Date(now.getTime() - i * interval * 60000)
    const h = t.getHours().toString().padStart(2, '0')
    const m = t.getMinutes().toString().padStart(2, '0')
    const r = Math.random()
    const status = r < 0.7 ? 'healthy' : r < 0.9 ? 'degraded' : 'down'
    items.push({ time: `${h}:${m}`, status })
  }
  return items
}

export default function HealthTimeline({ hours = 24, interval = 30 }: HealthTimelineProps) {
  const items = useMemo(() => generateTimeline(hours, interval), [hours, interval])

  const half = Math.floor(items.length / 2)

  function toAmPm(time: string) {
    const [h, m] = time.split(':').map(Number)
    const period = h >= 12 ? 'pm' : 'am'
    const hour = h % 12 || 12
    return `${hour}:${m.toString().padStart(2, '0')}${period}`
  }

  const markerPositions: Record<number, string> = {
    0: toAmPm(items[0].time),
    [half]: toAmPm(items[half].time),
    [items.length - 1]: 'Now',
  }

  return (
    <div className="flex flex-col w-full gap-[2px]">
      <div className="flex w-full gap-[2px]">
        {items.map((s, i) => {
          const barColor = s.status === 'healthy' ? 'var(--success)' : s.status === 'degraded' ? 'var(--warn)' : 'var(--error)'
          return (
            <div
              key={i}
              className="group relative flex-1"
            >
              <div
                className="w-full h-6 rounded-sm cursor-pointer transition-opacity hover:opacity-80"
                style={{ backgroundColor: barColor }}
              />
              <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1 hidden group-hover:block z-50">
                <div className="px-2 py-1 rounded text-xs whitespace-nowrap shadow" style={{ backgroundColor: 'var(--bg-primary)', color: 'var(--text-primary)', borderColor: 'var(--border-primary)', borderWidth: 1 }}>
                  {s.time} — {s.status}
                </div>
              </div>
            </div>
          )
        })}
      </div>
      <div className="flex w-full gap-[2px]">
        {items.map((_, i) => (
          <div key={i} className="flex-1">
            {markerPositions[i] && (
              <span className="text-sm" style={{ color: 'var(--text-secondary)' }}>{markerPositions[i]}</span>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}
