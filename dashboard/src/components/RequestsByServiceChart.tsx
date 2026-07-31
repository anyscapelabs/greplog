import ChartEmptyState from './ChartEmptyState.tsx'

interface RequestsByServiceChartProps {
  metric: string
}

export default function RequestsByServiceChart(_props: RequestsByServiceChartProps) {
  return <ChartEmptyState />
}
