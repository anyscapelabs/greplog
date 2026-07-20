import HealthTimeline from './HealthTimeline.tsx'

const serviceNames = ['api-gateway', 'auth-service', 'payment', 'db', 'worker']

export default function ServiceHealthChart() {
  return (
    <div className="flex flex-col gap-4 h-full px-2 pt-2">
      {serviceNames.map((name) => (
        <div key={name} className="flex items-center gap-2">
          <span className="text-xs w-24 shrink-0 truncate" style={{ color: 'var(--text-secondary)' }}>{name}</span>
          <div className="flex-1">
            <HealthTimeline hours={24} interval={60} showMarkers={false} />
          </div>
        </div>
      ))}
    </div>
  )
}
