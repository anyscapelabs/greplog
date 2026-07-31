import ChartEmptyState from './ChartEmptyState.tsx'

interface SystemMetricsChartProps {
  cpu: number[]
  memory: number[]
  diskIO: number[]
  network: number[]
}

export default function SystemMetricsChart(_props: SystemMetricsChartProps) {
  return <ChartEmptyState message="System metrics require OS-level agent collection \u2014 not yet implemented" />
}
