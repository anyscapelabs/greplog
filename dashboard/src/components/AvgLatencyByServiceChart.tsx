import ChartEmptyState from './ChartEmptyState.tsx'

interface AvgLatencyByServiceChartProps {
  metric: string
}

export default function AvgLatencyByServiceChart(_props: AvgLatencyByServiceChartProps) {
  return <ChartEmptyState />
}
