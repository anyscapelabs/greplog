import ChartEmptyState from './ChartEmptyState.tsx'

interface HealthTimelineProps {
  hours?: number
  interval?: number
  showMarkers?: boolean
}

export default function HealthTimeline(props: HealthTimelineProps) {
  void props
  return <ChartEmptyState />
}
