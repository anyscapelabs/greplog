import ChartEmptyState from './ChartEmptyState.tsx'

interface ErrorByServiceChartProps {
  metric?: string
  groupBy?: string
}

export default function ErrorByServiceChart(_props: ErrorByServiceChartProps) {
  return <ChartEmptyState />
}
