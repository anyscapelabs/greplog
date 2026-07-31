import ChartEmptyState from './ChartEmptyState.tsx'

interface StatusCodesChartProps {
  metric?: string
  groupBy?: string
}

export default function StatusCodesChart(_props: StatusCodesChartProps) {
  return <ChartEmptyState />
}
