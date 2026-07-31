import ChartEmptyState from './ChartEmptyState.tsx'

interface ErrorsChartProps {
  metric?: string
  groupBy?: string
}

export default function ErrorsChart(_props: ErrorsChartProps) {
  return <ChartEmptyState />
}
