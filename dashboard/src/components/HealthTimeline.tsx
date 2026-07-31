import ChartEmptyState from './ChartEmptyState.tsx'

interface HealthTimelineProps {
  hours?: number
  interval?: number
  showMarkers?: boolean
}

export default function HealthTimeline(_props: HealthTimelineProps) {
  return <ChartEmptyState />
}
