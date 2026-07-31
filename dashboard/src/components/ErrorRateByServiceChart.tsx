import ChartEmptyState from './ChartEmptyState.tsx'

interface ErrorRateByServiceChartProps {
  metric: string
}

export default function ErrorRateByServiceChart(_props: ErrorRateByServiceChartProps) {
  return <ChartEmptyState />
}
