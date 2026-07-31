import ChartEmptyState from './ChartEmptyState.tsx'

interface ErrorCountChartProps {
  metric?: string
  groupBy?: string
}

export default function ErrorCountChart(_props: ErrorCountChartProps) {
  return <ChartEmptyState />
}
