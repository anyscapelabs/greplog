# Dashboard Data Props — Complete Interface Specification

All data flows as props from a parent data layer. No hardcoded arrays, no inline mock data. This document specifies every prop interface for every component.

---

## Data Flow Hierarchy

```
DataProvider (or parent)
 ├── AnalyticsPage   ← metrics, charts, dropdowns
 ├── LogsPage        ← logs data, filter items, chart data, dropdowns
 │    ├── FilterSidebar    ← filter sections
 │    ├── LogsTable        ← rows, columns
 │    ├── Charts (3)       ← timeseries, status codes
 │    └── LogsDrawer       ← selected log
 ├── ErrorsPage      ← errors data, filter items, chart data, dropdowns
 │    ├── ErrorsFilterSidebar  ← filter sections
 │    ├── ErrorsTable         ← rows, columns
 │    ├── Charts (3)          ← timeseries, pie data
 │    └── ErrorsDrawer        ← selected error
 └── ServicesPage    ← services data, filter items, chart data, dropdowns
      ├── ServicesFilterSidebar  ← filter sections
      ├── ServiceCards            ← per-service metrics
      ├── ServicesTable          ← rows, columns
      ├── Charts (3)             ← per-service bar data
      └── ServicesDrawer         ← selected service
```

---

## 1. Page-Level Props

### AnalyticsPage

```typescript
interface AnalyticsPageProps {
  metrics: AnalyticsMetric[]           // 6 metric cards
  timeRanges: DropdownOption[]         // time range selector options
  services: DropdownOption[]           // service filter options
  autoRefreshOptions: DropdownOption[] // auto-refresh intervals
  ingestionTimeseries: TimeseriesPoint[]  // Log Ingestion chart
  errorRateTimeseries: TimeseriesPoint[]  // Error Over Time chart
  latencyData: {
    p50: number[]
    p90: number[]
    p99: number[]
  }
  serviceHealthData: ServiceHealthEntry[]  // Service Health stacked bar
  statusCodeDistribution: PieSlice[]       // Status Codes pie
  noisyServices: { name: string; count: number }[]
  severityDistribution: PieSlice[]
  systemMetrics: SystemMetric[]
  avgResponseTimes: { service: string; ms: number }[]
}

interface AnalyticsMetric {
  title: string
  value: string
  color: string
  rgb: string
  sparkline: number[]
}
```

### LogsPage

```typescript
interface LogsPageProps {
  logs: LogEntry[]
  totalLogs: number
  totalRows: number
  querySeconds: number
  filterSections: FilterSection[]       // FilterSidebar items
  charts: {
    volumeTimeseries: { timestamp: string; count: number }[]
    errorTimeseries: { timestamp: string; count: number }[]
    statusCodeDistribution: PieSlice[]
  }
  timeRanges: DropdownOption[]
  services: DropdownOption[]
  autoRefreshOptions: DropdownOption[]
  chartMetrics: DropdownOption[]         // Count, Rate
  groupByOptions: {
    requests: DropdownOption[]     // nothing, service, level
    errors: DropdownOption[]      // nothing, service, level, status_code
    statusCodes: DropdownOption[] // nothing, service, level
  }
  onViewLog: (log: LogEntry) => void
}

interface LogEntry {
  id: string
  timestamp: string
  level: 'info' | 'warn' | 'error' | 'debug' | 'critical'
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
```

### ErrorsPage

```typescript
interface ErrorsPageProps {
  errors: ErrorEntry[]
  totalErrors: number
  totalRows: number
  querySeconds: number
  filterSections: ErrorsFilterSection[]
  charts: {
    countTimeseries: { timestamp: string; count: number }[]
    rateTimeseries: { timestamp: string; rate: number }[]
    byServiceDistribution: PieSlice[]
  }
  timeRanges: DropdownOption[]
  services: DropdownOption[]
  autoRefreshOptions: DropdownOption[]
  chartMetrics: DropdownOption[]
  groupByOptions: {
    errorCount: DropdownOption[]
    errorRate: DropdownOption[]
    byService: DropdownOption[]
  }
  onViewError: (error: ErrorEntry) => void
}

interface ErrorEntry {
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
```

### ServicesPage

```typescript
interface ServicesPageProps {
  services: ServiceEntry[]
  totalRows: number
  querySeconds: number
  filterSections: ServicesFilterSection[]
  serviceCards: ServiceCardData[]
  charts: {
    requests: { service: string; count: number; rate: number }[]
    errorRates: { service: string; count: number; rate: number }[]
    latencies: { service: string; avg: number; p50: number; p95: number; p99: number }[]
  }
  timeRanges: DropdownOption[]
  autoRefreshOptions: DropdownOption[]
  countRateOptions: DropdownOption[]
  latencyOptions: DropdownOption[]
  onViewService: (service: ServiceEntry) => void
}

interface ServiceEntry {
  id: string
  name: string
  status: 'healthy' | 'degraded' | 'down'
  uptime: string
  requests: string
  errorRate: string
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

interface ServiceCardData {
  name: string
  label: string       // "2,500 req/s"
  sparkline: number[]
}
```

---

## 2. Filter Sidebar Props

### FilterSection (shared across all 3 sidebars)

```typescript
interface FilterSectionItem {
  id: string
  label: string
  count: number
  color?: string       // CSS var or hex for colored dots
}

interface FilterSectionConfig {
  id: string
  title: string
  items: FilterSectionItem[]
  defaultOpen?: boolean
  initialLimit?: number
}

interface FilterSidebarProps {
  sections: FilterSectionConfig[]
  checked: Record<string, boolean>
  onCheck: (id: string) => void
  width?: number        // default 280
  searchPlaceholder?: string
}
```

### Usage per page

**Logs FilterSidebar:**
```typescript
const logsFilterSections: FilterSectionConfig[] = [
  {
    id: 'response_status',
    title: 'response_status',
    defaultOpen: true,
    items: [
      { id: 'success', label: 'Success', count: 1243 },
      { id: 'redirect', label: 'Redirect', count: 89 },
      { id: 'client_error', label: 'Client Error', count: 342 },
      { id: 'server_error', label: 'Server Error', count: 27 },
    ]
  },
  {
    id: 'status_code',
    title: 'status_code',
    items: [
      { id: '200', label: '200', count: 892 },
      // ... etc
    ]
  },
  {
    id: 'log_level',
    title: 'log_level',
    defaultOpen: true,
    items: [
      { id: 'error', label: 'Error', count: 27, color: 'var(--error)' },
      { id: 'warn', label: 'Warn', count: 156, color: 'var(--warn)' },
      { id: 'info', label: 'Info', count: 1243, color: 'var(--info)' },
      { id: 'debug', label: 'Debug', count: 3456, color: 'var(--text-secondary)' },
    ]
  },
  {
    id: 'service_name',
    title: 'service_name',
    items: [
      { id: 'web', label: 'web', count: 2341 },
      { id: 'api', label: 'api', count: 1567 },
      { id: 'db', label: 'db', count: 892 },
      { id: 'worker', label: 'worker', count: 423 },
    ]
  },
]
```

**ErrorsFilterSidebar:**
```typescript
// Reuses same FilterSectionConfig[] shape
// Sections: service_name, error_type, log_level, status_code
// Items include error_type with color dots, log_level with colors
```

**ServicesFilterSidebar:**
```typescript
// Sections: health_status (with color dots), service_name, environment
```

---

## 3. Table Props

```typescript
interface TableColumn {
  key: string
  label: string
  width: string        // tailwind class e.g. 'w-[180px]'
  sortable?: boolean   // default true
}

interface TableProps<T> {
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
  limits?: string[]        // default ['500', '1k', '5k', '10k']
  exportFilename?: string  // default 'export.csv'
}
```

**LogsTable** uses `TableProps<LogEntry>` with columns:
```
timestamp (180px), level (100px), service (150px), statusCode (130px),
message (360px), response (130px), logger (150px), correlationId (150px), file (180px)
```

**ErrorsTable** uses `TableProps<ErrorEntry>` with columns:
```
timestamp (180px), level (90px), service (130px), errorCode (160px),
message (400px), latency (100px), freq (90px)
```

**ServicesTable** uses `TableProps<ServiceEntry>` with columns:
```
service (180px), status (120px), uptime (110px), requests (140px),
errorRate (130px), avgLatency (130px), p95 (120px), p99 (120px), lastSeen (170px)
```

---

## 4. Chart Props

```typescript
// Shared
interface TimeseriesPoint {
  timestamp: string
  value: number
  group?: string
}

interface PieSlice {
  name: string
  value: number
  color: string
}

interface BarDataPoint {
  label: string
  value: number
  color?: string
}

// --- Analytics Page Charts ---

interface IngestionChartProps {
  data: TimeseriesPoint[]
}

interface ErrorOverTimeChartProps {
  data: TimeseriesPoint[]
}

interface LatencyPercentilesChartProps {
  p50: number[]
  p90: number[]
  p99: number[]
  labels: string[]
}

interface ServiceHealthChartProps {
  services: {
    name: string
    healthy: number
    degraded: number
    down: number
  }[]
}

interface StatusCodesPieChartProps {
  data: PieSlice[]
}

interface NoisyServicesChartProps {
  data: BarDataPoint[]
}

interface SeverityChartProps {
  data: PieSlice[]
}

interface SystemMetricsChartProps {
  cpu: number[]
  memory: number[]
  diskIO: number[]
  network: number[]
}

interface AvgResponseTimeChartProps {
  data: BarDataPoint[]
}

// --- Logs Page Charts ---

interface LogVolumeChartProps {
  metric: string
  groupBy: string
  data: { timestamp: string; total: number }[]
  groupedData?: Record<string, { timestamp: string; value: number }[]>
}

interface ErrorsChartProps {
  metric: string
  groupBy: string
  data: { timestamp: string; count: number }[]
  groupedData?: Record<string, { timestamp: string; value: number }[]>
}

interface StatusCodesChartProps {
  metric: string
  groupBy: string
  data: PieSlice[]
  groupedData?: Record<string, PieSlice[]>
}

// --- Errors Page Charts ---

interface ErrorCountChartProps {
  metric: string
  groupBy: string
  data: { timestamp: string; count: number }[]
  groupedData?: Record<string, { timestamp: string; value: number }[]>
}

interface ErrorRateChartProps {
  metric: string
  groupBy: string
  data: { timestamp: string; rate: number }[]
  groupedData?: Record<string, { timestamp: string; value: number }[]>
}

interface ErrorByServiceChartProps {
  metric: string
  groupBy: string
  data: PieSlice[]
  groupedData?: Record<string, PieSlice[]>
}

// --- Services Page Charts ---

interface RequestsByServiceChartProps {
  metric: string
  data: BarDataPoint[]
}

interface ErrorRateByServiceChartProps {
  metric: string
  data: BarDataPoint[]
}

interface AvgLatencyByServiceChartProps {
  metric: string
  data: BarDataPoint[]
}
```

---

## 5. Drawer Props

```typescript
interface DrawerProps {
  open: boolean
  onClose: () => void
  width?: string    // default 'w-[600px]'
}
```

### LogsDrawer

```typescript
interface LogsDrawerProps extends DrawerProps {
  log: LogEntry | null
}

// Fields shown:
// Header: level badge, log ID (copyable), timestamp
// Message: full text
// Context: service, statusCode, response, logger, file (clickable), correlationId (copyable), environment, host
// StackTrace: collapsible, monospace (only for error/critical)
// RawPayload: collapsible JSON (expanded for debug)
// RelatedErrors: ErrorEntry[] (when correlationId present)
// Footer: Copy ID, Copy JSON, View Related Errors, View in Logs
```

### ErrorsDrawer

```typescript
interface ErrorsDrawerProps extends DrawerProps {
  error: ErrorEntry | null
}

// Fields shown:
// Header: severity badge, error ID (copyable), errorCode, timestamp
// ErrorSummary: full message, frequency, firstSeen, lastSeen
// StackTrace: collapsible, monospace
// AffectedEndpoints: { method, path, count }[]
// Footer: Copy ID, View Related Logs
```

### ServicesDrawer

```typescript
interface ServicesDrawerProps extends DrawerProps {
  service: ServiceEntry | null
}

// Fields shown:
// Header: service name, health dot, uptime, request throughput
// PerformanceMetrics: 4 mini cards (requests, errorRate, avgLatency, p95Latency) with sparklines
// HealthTimeline: colored dot timeline
// ServiceDetails: id (copyable), environment, hosts, version, uptime, lastDeployed, firstSeen
// RecentErrors: ErrorEntry[] (last 5)
// RelatedLogs: LogEntry[] (last 5)
// Footer: View Logs, View Errors, View in Analytics
```

---

## 6. Shared Utility Props

```typescript
interface DropdownOption {
  label: string
  value: string
}

interface AnalyticsChartPanelProps {
  title: string
  children: ReactNode
  dropdownItems?: DropdownOption[]
  dropdownValue?: string
  onDropdownChange?: (value: string) => void
  height?: string
}

interface AnalyticsMetricCardProps {
  title: string
  value: string
  color: string
  rgb: string
  data: number[]    // sparkline
}

interface ServiceCardProps {
  name: string
  label: string
  sparkline: number[]
}

interface PageHeaderProps {
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
  extraActions?: ReactNode
}

interface ChartCardProps {
  title: string
  children: ReactNode
  className?: string
}
```

---

## Migration: Files to Convert

| Priority | File | Current Hardcoded Data | Replace With |
|----------|------|----------------------|--------------|
| 🔴 High | `LogsTable.tsx` | 50k mock rows, columns, limits, export name | `data: LogEntry[]`, `columns: TableColumn[]` |
| 🔴 High | `ErrorsTable.tsx` | 50k mock rows, columns, limits, export name | `data: ErrorEntry[]`, `columns: TableColumn[]` |
| 🔴 High | `ServicesTable.tsx` | 50k mock rows, columns, limits, export name | `data: ServiceEntry[]`, `columns: TableColumn[]` |
| 🔴 High | `FilterSidebar.tsx` | 4 sections with hardcoded items/counts | `sections: FilterSectionConfig[]` |
| 🔴 High | `ErrorsFilterSidebar.tsx` | 4 sections with hardcoded items/counts | `sections: FilterSectionConfig[]` |
| 🔴 High | `ServicesFilterSidebar.tsx` | 3 sections with hardcoded items/counts | `sections: FilterSectionConfig[]` |
| 🟡 Medium | `Analytics.tsx` | metricsData, generateData() | `metrics, ingestionData, errorData, ...` |
| 🟡 Medium | `Logs.tsx` | chart options, services list, timeRanges | All as page-level props |
| 🟡 Medium | `Errors.tsx` | chart options, services list, timeRanges | All as page-level props |
| 🟡 Medium | `Services.tsx` | serviceCards, generateData() | `services, serviceCards, chart data` |
| 🟡 Medium | All chart components | Random data, hardcoded arrays | `data` props from parent |
| 🟢 Low | `LogsDrawer.tsx` | Hardcoded stack trace, payload, related | `log: LogEntry` prop only |
| 🟢 Low | `ErrorsDrawer.tsx` | Hardcoded stack trace, endpoints | `error: ErrorEntry` prop only |
| 🟢 Low | `ServicesDrawer.tsx` | Hardcoded details, errors, logs | `service: ServiceEntry` prop only |