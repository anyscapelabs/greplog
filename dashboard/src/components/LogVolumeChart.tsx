import ChartEmptyState from './ChartEmptyState.tsx'

interface LogVolumeChartProps {
  metric?: string
  groupBy?: string
}

export default function LogVolumeChart(_props: LogVolumeChartProps) {
  return <ChartEmptyState />
}
