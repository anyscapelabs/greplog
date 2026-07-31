import ChartEmptyState from './ChartEmptyState.tsx'

interface ErrorRateChartProps {
  metric?: string
  groupBy?: string
}

export default function ErrorRateChart(_props: ErrorRateChartProps) {
  return <ChartEmptyState />
}
