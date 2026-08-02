/* ── Shared Primitives ── */

export interface DropdownOption {
  label: string
  value: string
}

export interface TimeseriesPoint {
  timestamp: string
  value: number
  group?: string
}

export interface PieSlice {
  name: string
  value: number
  color: string
}

export interface BarDataPoint {
  label: string
  value: number
  color?: string
}

/* ── Filter Sidebar ── */

export interface FilterSectionItem {
  id: string
  label: string
  count: number
  color?: string
}

export interface FilterSectionConfig {
  id: string
  title: string
  items: FilterSectionItem[]
  defaultOpen?: boolean
  initialLimit?: number
}

export interface FilterSidebarProps {
  sections: FilterSectionConfig[]
  checked: Record<string, boolean>
  onCheck: (id: string) => void
  width?: number
  searchPlaceholder?: string
}

export interface AnalyticsMetric {
  title: string
  value: string
  color: string
  rgb: string
  sparkline: number[]
}

export interface ServiceHealthEntry {
  name: string
  healthy: number
  degraded: number
  down: number
}

export interface SystemMetric {
  cpu: number[]
  memory: number[]
  diskIO: number[]
  network: number[]
}

/* ── Logs ── */

export type LogLevel = 'info' | 'warn' | 'error' | 'debug' | 'critical'

export interface LogEntry {
  id: string
  timestamp: string
  level: LogLevel
  service: string
  statusCode: number
  message: string
  response: string
  logger: string
  correlationId: string
  file: string
  environment?: string
  host?: string
  rawPayload?: Record<string, unknown>
  stackTrace?: string
}

export interface LogCharts {
  volumeTimeseries: TimeseriesPoint[]
  errorTimeseries: { timestamp: string; count: number }[]
  statusCodeDistribution: PieSlice[]
  logsHistogram: { timestamp: string; count: number }[]
}

export interface LogsPageProps {
  logs: LogEntry[]
  totalLogs: number
  totalRows: number
  querySeconds: number
  isWaiting: boolean
  filterSections: FilterSectionConfig[]
  charts: LogCharts
  timeRanges: DropdownOption[]
  services: DropdownOption[]
  autoRefreshOptions: DropdownOption[]
  chartMetrics: DropdownOption[]
  groupByOptions: {
    requests: DropdownOption[]
    errors: DropdownOption[]
    statusCodes: DropdownOption[]
  }
  onViewLog?: (log: LogEntry) => void
  refetch: () => void
  manualRefetch?: () => void
}

/* ── Errors ── */

export interface ErrorEntry {
  id: string
  timestamp: string
  errorCode: number
  freq: number
  level: 'error' | 'critical' | 'warn'
  latency: string
  service: string
  message: string
  stackTrace?: string
  errorType?: string
  firstSeen?: string
  lastSeen?: string
  correlationId?: string
  affectedEndpoints?: { method: string; path: string; count: number }[]
}

export interface ErrorCharts {
  countTimeseries: TimeseriesPoint[]
  rateTimeseries: { timestamp: string; rate: number }[]
  byServiceDistribution: PieSlice[]
}

export interface ErrorsPageProps {
  errors: ErrorEntry[]
  totalErrors: number
  totalRows: number
  querySeconds: number
  isWaiting: boolean
  filterSections: FilterSectionConfig[]
  charts: ErrorCharts
  timeRanges: DropdownOption[]
  services: DropdownOption[]
  autoRefreshOptions: DropdownOption[]
  chartMetrics: DropdownOption[]
  groupByOptions: {
    errorCount: DropdownOption[]
    errorRate: DropdownOption[]
    byService: DropdownOption[]
  }
  onViewError?: (error: ErrorEntry) => void
  refetch: () => void
  manualRefetch?: () => void
}

/* ── Services ── */

export type ServiceStatus = 'active' | 'detected_only'

export type ServiceHealth = 'healthy' | 'degraded' | 'unhealthy' | 'unknown'

const HEALTH_THRESHOLDS = {
  healthy: 0.01,
  degraded: 0.05,
} as const

export function classifyHealth(errorRate: number, eventCount: number): ServiceHealth {
  if (eventCount === 0) return 'unknown'
  if (errorRate < HEALTH_THRESHOLDS.healthy) return 'healthy'
  if (errorRate < HEALTH_THRESHOLDS.degraded) return 'degraded'
  return 'unhealthy'
}

export interface ServiceEntry {
  id: string
  name: string
  status: ServiceStatus
  health: ServiceHealth
  errorRate: number
  eventCount: number
  uptime: string
  requests: string
  avgLatency: string
  p95: string
  p99: string
  lastSeen: string
  environment?: string
  version?: string
  hosts?: string[]
  firstSeen?: string
  lastDeployed?: string
  healthTimeline?: { timestamp: string; status: string }[]
}

export interface ServiceCardData {
  name: string
  label: string
  sparkline: number[]
}

export interface ServiceCharts {
  requests: { service: string; count: number; rate: number }[]
  errorRates: { service: string; count: number; rate: number }[]
  latencies: { service: string; avg: number; p50: number; p95: number; p99: number }[]
}

export interface ServicesPageProps {
  services: ServiceEntry[]
  totalRows: number
  querySeconds: number
  isWaiting: boolean
  filterSections: FilterSectionConfig[]
  serviceCards: ServiceCardData[]
  charts: ServiceCharts
  timeRanges: DropdownOption[]
  autoRefreshOptions: DropdownOption[]
  countRateOptions: DropdownOption[]
  latencyOptions: DropdownOption[]
  onViewService?: (service: ServiceEntry) => void
  refetch: () => void
  manualRefetch?: () => void
}

/* ── Analytics ── */

export interface AnalyticsPageProps {
  metrics: AnalyticsMetric[]
  isWaiting: boolean
  timeRanges: DropdownOption[]
  services: DropdownOption[]
  autoRefreshOptions: DropdownOption[]
  ingestionTimeseries: TimeseriesPoint[]
  errorRateTimeseries: TimeseriesPoint[]
  latencyData: {
    p50: number[]
    p90: number[]
    p99: number[]
  }
  serviceHealthData: ServiceHealthEntry[]
  statusCodeDistribution: PieSlice[]
  noisyServices: { name: string; count: number }[]
  severityDistribution: PieSlice[]
  systemMetrics: SystemMetric
  avgResponseTimes: { service: string; ms: number }[]
  ingestionOptions: DropdownOption[]
  rateCountOptions: DropdownOption[]
  latencyOptions: DropdownOption[]
  sortOptions: DropdownOption[]
  ingestionMetric: string
  onIngestionMetricChange: (v: string) => void
  errorRateMetric: string
  onErrorRateMetricChange: (v: string) => void
  latencyView: string
  onLatencyViewChange: (v: string) => void
  statusCodeMetric: string
  onStatusCodeMetricChange: (v: string) => void
  noisySort: string
  onNoisySortChange: (v: string) => void
  refetch: () => void
  manualRefetch?: () => void
}

/* ── Drawer Props ── */

export interface DrawerProps {
  open: boolean
  onClose: () => void
  width?: string
}

export interface LogsDrawerProps extends DrawerProps {
  log: LogEntry | null
}

export interface ErrorsDrawerProps extends DrawerProps {
  error: ErrorEntry | null
}

export interface ServicesDrawerProps extends DrawerProps {
  service: ServiceEntry | null
}

/* ── Table ── */

export interface TableColumn {
  key: string
  label: string
  width: string
  sortable?: boolean
}

export interface TableProps<T> {
  columns: TableColumn[]
  data: T[]
  totalRows: number
  totalLogs: number
  querySeconds: number
  limit: string
  onLimitChange: (limit: string) => void
  sortColumn: string | null
  sortDirection: 'asc' | 'desc' | null
  onSort: (column: string) => void
  page: number
  onPageChange: (page: number) => void
  onExport: () => void
  onView: (row: T) => void
  limits?: string[]
  exportFilename?: string
}

/* ── Chart Props ── */

export interface IngestionChartProps {
  data: TimeseriesPoint[]
}

export interface ErrorOverTimeChartProps {
  data: TimeseriesPoint[]
}

export interface LatencyPercentilesChartProps {
  p50: number[]
  p90: number[]
  p99: number[]
  labels: string[]
}

export interface ServiceHealthChartProps {
  services: ServiceHealthEntry[]
}

export interface StatusCodesPieChartProps {
  data: PieSlice[]
}

export interface NoisyServicesChartProps {
  data: BarDataPoint[]
}

export interface SeverityChartProps {
  data: PieSlice[]
}

export interface SystemMetricsChartProps {
  cpu: number[]
  memory: number[]
  diskIO: number[]
  network: number[]
}

export interface AvgResponseTimeChartProps {
  data: BarDataPoint[]
}

export interface ErrorCountChartProps {
  metric: string
  groupBy: string
  data: TimeseriesPoint[]
}

export interface ErrorRateChartProps {
  metric: string
  groupBy: string
  data: { timestamp: string; rate: number }[]
}

export interface ErrorByServiceChartProps {
  metric: string
  groupBy: string
  data: PieSlice[]
}

export interface RequestsByServiceChartProps {
  metric: string
  data: BarDataPoint[]
}

export interface ErrorRateByServiceChartProps {
  metric: string
  data: BarDataPoint[]
}

export interface AvgLatencyByServiceChartProps {
  metric: string
  data: BarDataPoint[]
}

export interface LogsHistogramChartProps {
  data: { timestamp: string; count: number }[]
}

export interface AnalyticsChartPanelProps {
  title: string
  children: React.ReactNode
  dropdownItems?: DropdownOption[]
  dropdownValue?: string
  onDropdownChange?: (value: string) => void
  height?: string
}

export interface AnalyticsMetricCardProps {
  title: string
  value: string
  color: string
  rgb: string
  data: number[]
}

export interface ServiceCardProps {
  name: string
  label: string
  sparkline: number[]
}

export interface PageHeaderProps {
  title: string
  timeRange: string
  onTimeRangeChange: (value: string) => void
  autoRefresh: string
  onAutoRefreshChange: (value: string) => void
  timeRanges: DropdownOption[]
  autoRefreshOptions: DropdownOption[]
  services?: DropdownOption[]
  service?: string
  onServiceChange?: (value: string) => void
  showLive?: boolean
  showFilterToggle?: boolean
  filterOpen?: boolean
  onFilterToggle?: () => void
  chips?: string[]
  query?: string
  onQueryChange?: (value: string) => void
  onQueryKeyDown?: (e: React.KeyboardEvent) => void
  onRemoveChip?: (chip: string) => void
  searchPlaceholder?: string
  extraActions?: React.ReactNode
}