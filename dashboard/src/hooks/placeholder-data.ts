import type {
  LogEntry, ErrorEntry, ServiceEntry, ServiceCardData, ServiceCharts,
  AnalyticsMetric, ServiceHealthEntry, PieSlice, SystemMetric,
  FilterSectionConfig, TimeseriesPoint, LogCharts, ErrorCharts,
} from '../types/index.ts'

/* ── Helpers ── */

function generateData(base: number, variance: number, min = 0, length = 60): number[] {
  let current = base
  return Array.from({ length }, () => {
    current += (Math.random() - 0.5) * variance
    if (current < min) current = min
    return Math.round(current * 100) / 100
  })
}

function generateTimestamps(count: number, intervalMs = 60000): string[] {
  const now = Date.now()
  return Array.from({ length: count }, (_, i) =>
    new Date(now - (count - 1 - i) * intervalMs).toISOString()
  )
}

/* ── Analytics Placeholder Data ── */

export const placeholderAnalyticsMetrics: AnalyticsMetric[] = [
  { title: 'Requests', value: '1.2M', color: '#3b82f6', rgb: '59, 130, 246', sparkline: generateData(200, 50) },
  { title: 'Error rate', value: '0.12%', color: '#dc2626', rgb: '220, 38, 38', sparkline: generateData(1, 0.5) },
  { title: 'P.95 latency', value: '145ms', color: '#d97706', rgb: '217, 119, 6', sparkline: generateData(145, 10) },
  { title: 'Request throughput', value: '2.4k/s', color: '#16a34a', rgb: '22, 163, 74', sparkline: generateData(2400, 200) },
  { title: 'Active services', value: '42', color: '#2563eb', rgb: '37, 99, 235', sparkline: generateData(42, 2) },
  { title: 'Trace volume', value: '840GB', color: '#8b5cf6', rgb: '139, 92, 246', sparkline: generateData(800, 40) },
]

export function placeholderIngestionTimeseries(): TimeseriesPoint[] {
  const timestamps = generateTimestamps(120)
  return timestamps.map((ts, i) => ({
    timestamp: ts,
    value: Math.round(200 + Math.sin(i * 0.1) * 50 + (Math.random() - 0.5) * 40),
  }))
}

export function placeholderErrorRateTimeseries(): TimeseriesPoint[] {
  const timestamps = generateTimestamps(120)
  return timestamps.map((ts) => ({
    timestamp: ts,
    value: Math.round((Math.random() * 0.5 + 0.01) * 100) / 100,
  }))
}

export const placeholderLatencyData = {
  p50: generateData(50, 10, 0, 120),
  p90: generateData(100, 15, 0, 120),
  p99: generateData(200, 25, 0, 120),
}

export const placeholderServiceHealth: ServiceHealthEntry[] = [
  { name: 'web', healthy: 85, degraded: 10, down: 5 },
  { name: 'api', healthy: 92, degraded: 6, down: 2 },
  { name: 'db', healthy: 78, degraded: 15, down: 7 },
  { name: 'worker', healthy: 95, degraded: 4, down: 1 },
  { name: 'auth', healthy: 88, degraded: 8, down: 4 },
]

export const placeholderStatusCodeDistribution: PieSlice[] = [
  { name: '2xx', value: 8200, color: '#22c55e' },
  { name: '3xx', value: 1200, color: '#3b82f6' },
  { name: '4xx', value: 800, color: '#eab308' },
  { name: '5xx', value: 200, color: '#ef4444' },
]

export const placeholderNoisyServices = [
  { name: 'api', count: 1250 },
  { name: 'web', count: 980 },
  { name: 'worker', count: 340 },
  { name: 'db', count: 120 },
  { name: 'auth', count: 45 },
]

export const placeholderSeverityDistribution: PieSlice[] = [
  { name: 'Info', value: 4500, color: '#3b82f6' },
  { name: 'Warn', value: 800, color: '#eab308' },
  { name: 'Error', value: 250, color: '#f97316' },
  { name: 'Critical', value: 45, color: '#ef4444' },
]

export const placeholderSystemMetrics: SystemMetric = {
  cpu: generateData(60, 15, 0, 120),
  memory: generateData(70, 10, 0, 120),
  diskIO: generateData(40, 20, 0, 120),
  network: generateData(500, 100, 0, 120),
}

export const placeholderAvgResponseTimes = [
  { service: 'api', ms: 45 },
  { service: 'web', ms: 120 },
  { service: 'db', ms: 15 },
  { service: 'worker', ms: 340 },
  { service: 'auth', ms: 85 },
]

/* ── Logs Placeholder Data ── */

const sampleLogMessages = [
  'GET /users/1234 returned 200 OK in 45ms',
  'POST /orders failed with 500 Internal Server Error',
  'Database connection pool exhausted after 30s timeout',
  'Cache miss for key user:session:abcd, fetching from origin',
  'User authentication succeeded for user@example.com',
  'Rate limit exceeded for IP 192.168.1.1, returning 429',
  'Webhook delivery failed to endpoint https://hooks.example.com/callback',
  'Background job processed 250 records in 1.2s',
  'TLS handshake completed with client cert CN=*.example.com',
  'Request body exceeded maximum size of 10MB, returning 413',
  'Circuit breaker opened for downstream service payments after 5 failures',
  'Health check passed for service web on port 8080',
  'Migration v42 applied successfully (230ms)',
  'DNS resolution failed for service-discovery.internal',
  'JWT token expired for user session abc-123-def, requesting refresh',
]

const loggers = ['http', 'db', 'cache', 'auth', 'worker', 'circuit-breaker', 'health', 'migration', 'dns', 'jwt']
const files = [
  '/app/routes/users.ts:142',
  '/app/services/orders.ts:89',
  '/app/lib/database.ts:56',
  '/app/middleware/auth.ts:34',
  '/app/workers/job-processor.ts:201',
  '/app/lib/cache.ts:78',
  '/app/middleware/rate-limit.ts:45',
  '/app/handlers/webhook.ts:112',
  '/app/lib/http.ts:67',
  '/app/services/payments.ts:203',
  '/app/health/check.ts:23',
  '/app/db/migrate.ts:88',
  '/app/lib/discovery.ts:55',
  '/app/middleware/jwt.ts:41',
]

function generateLogEntry(id: number, timestamp: string): LogEntry {
  const levels: LogEntry['level'][] = ['info', 'warn', 'error', 'debug', 'critical']
  const level = levels[Math.floor(Math.random() * (id % 7 === 0 ? 5 : 3))]
  const services = ['web', 'api', 'db', 'worker']
  const service = services[Math.floor(Math.random() * services.length)]
  const msgIndex = Math.floor(Math.random() * sampleLogMessages.length)
  return {
    id: `log-${id}`,
    timestamp,
    level,
    service,
    statusCode: level === 'error' || level === 'critical' ? 500 : level === 'warn' ? 429 : 200,
    message: sampleLogMessages[msgIndex],
    response: `${Math.floor(Math.random() * 200 + 10)}ms`,
    logger: loggers[Math.floor(Math.random() * loggers.length)],
    correlationId: `corr-${Math.random().toString(36).slice(2, 10)}`,
    file: files[Math.floor(Math.random() * files.length)],
    environment: 'production',
    host: `ip-10-0-${Math.floor(Math.random() * 255)}-${Math.floor(Math.random() * 255)}`,
    stackTrace: level === 'error' || level === 'critical'
      ? `Error: ${sampleLogMessages[msgIndex]}\n    at Object.<anonymous> (${files[Math.floor(Math.random() * files.length)]}:1)\n    at Generator.next (<anonymous>)\n    at fulfilled (${files[Math.floor(Math.random() * files.length)]}:1)`
      : undefined,
  }
}

export function placeholderLogs(count = 50000): LogEntry[] {
  const timestamps = generateTimestamps(count, 100)
  return Array.from({ length: count }, (_, i) => generateLogEntry(i + 1, timestamps[i]))
}

export const placeholderLogFilterSections: FilterSectionConfig[] = [
  {
    id: 'response_status',
    title: 'response_status',
    defaultOpen: true,
    items: [
      { id: 'success', label: 'Success', count: 1243 },
      { id: 'redirect', label: 'Redirect', count: 89 },
      { id: 'client_error', label: 'Client Error', count: 342 },
      { id: 'server_error', label: 'Server Error', count: 27 },
    ],
  },
  {
    id: 'status_code',
    title: 'status_code',
    items: [
      { id: '200', label: '200', count: 892 },
      { id: '201', label: '201', count: 156 },
      { id: '301', label: '301', count: 34 },
      { id: '404', label: '404', count: 89 },
      { id: '429', label: '429', count: 67 },
      { id: '500', label: '500', count: 27 },
    ],
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
    ],
  },
  {
    id: 'service_name',
    title: 'service_name',
    items: [
      { id: 'web', label: 'web', count: 2341 },
      { id: 'api', label: 'api', count: 1567 },
      { id: 'db', label: 'db', count: 892 },
      { id: 'worker', label: 'worker', count: 423 },
    ],
  },
]

export function placeholderLogCharts(): LogCharts {
  const timestamps = generateTimestamps(60)
  return {
    volumeTimeseries: timestamps.map((ts) => ({ timestamp: ts, value: Math.round(Math.random() * 500 + 100) })),
    errorTimeseries: timestamps.map((ts) => ({ timestamp: ts, count: Math.round(Math.random() * 20) })),
    statusCodeDistribution: placeholderStatusCodeDistribution,
    logsHistogram: {
      buckets: timestamps.map((ts) => new Date(ts).toISOString().slice(11, 16)),
      levels: [
        { level: 'info', counts: Array.from({ length: 60 }, () => Math.round(Math.random() * 120)) },
        { level: 'warn', counts: Array.from({ length: 60 }, () => Math.round(Math.random() * 40)) },
        { level: 'error', counts: Array.from({ length: 60 }, () => Math.round(Math.random() * 15)) },
      ],
    },
  }
}

/* ── Errors Placeholder Data ── */

const sampleErrorMessages = [
  'Internal Server Error: Cannot read properties of undefined',
  'Database query timeout after 30s',
  'Connection refused: service payments unavailable',
  'Failed to authenticate user: invalid token signature',
  'Rate limit exceeded for endpoint /api/orders',
  'Out of memory: cannot allocate buffer of size 1048576',
  'Deadlock detected on table orders_lock',
  'TLS certificate expired for *.example.com',
  'Segmentation fault in native module image-processor',
]

function generateErrorEntry(id: number, timestamp: string): ErrorEntry {
  const levels: ErrorEntry['level'][] = ['error', 'critical', 'warn']
  const level = levels[Math.floor(Math.random() * levels.length)]
  const services = ['web', 'api', 'db', 'worker']
  const service = services[Math.floor(Math.random() * services.length)]
  const msgIndex = Math.floor(Math.random() * sampleErrorMessages.length)
  return {
    id: `err-${id}`,
    timestamp,
    errorCode: [500, 503, 504, 408, 429, 401, 403][Math.floor(Math.random() * 7)],
    freq: Math.floor(Math.random() * 100 + 1),
    level,
    latency: `${Math.floor(Math.random() * 500 + 50)}ms`,
    service,
    message: sampleErrorMessages[msgIndex],
    stackTrace: `Error: ${sampleErrorMessages[msgIndex]}\n    at Object.<anonymous> (/app/services/${service}/handler.ts:42)\n    at Generator.next (<anonymous>)\n    at fulfilled (/app/lib/async.ts:15)`,
    errorType: ['RuntimeError', 'TimeoutError', 'ConnectionError', 'AuthError', 'MemoryError'][Math.floor(Math.random() * 5)],
    firstSeen: new Date(Date.now() - Math.random() * 86400000 * 7).toISOString(),
    lastSeen: timestamp,
    correlationId: `corr-${Math.random().toString(36).slice(2, 10)}`,
    affectedEndpoints: [
      { method: 'GET', path: `/api/${service}/users`, count: Math.floor(Math.random() * 50 + 1) },
      { method: 'POST', path: `/api/${service}/orders`, count: Math.floor(Math.random() * 30 + 1) },
    ],
  }
}

export function placeholderErrors(count = 50000): ErrorEntry[] {
  const timestamps = generateTimestamps(count, 120)
  return Array.from({ length: count }, (_, i) => generateErrorEntry(i + 1, timestamps[i]))
}

export const placeholderErrorFilterSections: FilterSectionConfig[] = [
  {
    id: 'service_name',
    title: 'service_name',
    defaultOpen: true,
    items: [
      { id: 'web', label: 'web', count: 345 },
      { id: 'api', label: 'api', count: 567 },
      { id: 'db', label: 'db', count: 234 },
      { id: 'worker', label: 'worker', count: 89 },
    ],
  },
  {
    id: 'error_type',
    title: 'error_type',
    defaultOpen: true,
    items: [
      { id: 'runtime', label: 'RuntimeError', count: 267, color: '#ef4444' },
      { id: 'timeout', label: 'TimeoutError', count: 189, color: '#f97316' },
      { id: 'connection', label: 'ConnectionError', count: 145, color: '#eab308' },
      { id: 'auth', label: 'AuthError', count: 78, color: '#8b5cf6' },
      { id: 'memory', label: 'MemoryError', count: 23, color: '#dc2626' },
    ],
  },
  {
    id: 'log_level',
    title: 'log_level',
    items: [
      { id: 'error', label: 'Error', count: 567, color: 'var(--error)' },
      { id: 'critical', label: 'Critical', count: 89, color: '#dc2626' },
      { id: 'warn', label: 'Warn', count: 234, color: 'var(--warn)' },
    ],
  },
  {
    id: 'status_code',
    title: 'status_code',
    items: [
      { id: '500', label: '500', count: 312 },
      { id: '503', label: '503', count: 134 },
      { id: '504', label: '504', count: 56 },
      { id: '408', label: '408', count: 78 },
      { id: '429', label: '429', count: 145 },
    ],
  },
]

export function placeholderErrorCharts(): ErrorCharts {
  const timestamps = generateTimestamps(60)
  return {
    countTimeseries: timestamps.map((ts) => ({ timestamp: ts, value: Math.round(Math.random() * 30 + 5) })),
    rateTimeseries: timestamps.map((ts) => ({ timestamp: ts, rate: Math.round((Math.random() * 0.5 + 0.01) * 100) / 100 })),
    byServiceDistribution: [
      { name: 'api', value: 567, color: '#3b82f6' },
      { name: 'web', value: 345, color: '#22c55e' },
      { name: 'db', value: 234, color: '#eab308' },
      { name: 'worker', value: 89, color: '#ef4444' },
    ],
  }
}

/* ── Services Placeholder Data ── */

function generateServiceEntry(name: string, uptime: string, requests: string, errorRate: number, avg: string, p95v: string, p99v: string, status: ServiceEntry['status'], lastSeen: string): ServiceEntry {
  return {
    id: `svc-${name}`,
    name,
    status,
    health: 'unknown',
    errorRate,
    eventCount: 0,
    uptime,
    requests,
    avgLatency: avg,
    p95: p95v,
    p99: p99v,
    lastSeen,
    environment: 'production',
    version: `v${Math.floor(Math.random() * 5) + 1}.${Math.floor(Math.random() * 10)}.${Math.floor(Math.random() * 20)}`,
    hosts: [`ip-10-0-${Math.floor(Math.random() * 255)}-${Math.floor(Math.random() * 255)}`, `ip-10-0-${Math.floor(Math.random() * 255)}-${Math.floor(Math.random() * 255)}`],
    firstSeen: new Date(Date.now() - 86400000 * 30).toISOString(),
    lastDeployed: new Date(Date.now() - Math.random() * 86400000 * 3).toISOString(),
    healthTimeline: Array.from({ length: 24 }, (_, i) => ({
      timestamp: new Date(Date.now() - (23 - i) * 3600000).toISOString(),
      status: (['healthy', 'healthy', 'healthy', 'degraded', 'healthy', 'down'] as const)[Math.floor(Math.random() * 6)],
    })),
  }
}

const now = new Date().toISOString()

export const placeholderServiceEntries: ServiceEntry[] = [
  generateServiceEntry('api', '99.98%', '2,500 req/s', 0.0012, '45ms', '120ms', '250ms', 'active', now),
  generateServiceEntry('web', '99.95%', '1,850 req/s', 0.0008, '120ms', '340ms', '650ms', 'active', now),
  generateServiceEntry('db', '99.89%', '940 req/s', 0.0034, '15ms', '45ms', '120ms', 'active', now),
  generateServiceEntry('worker', '99.97%', '250 req/s', 0.0005, '340ms', '890ms', '2.1s', 'detected_only', now),
  generateServiceEntry('auth', '99.99%', '340 req/s', 0.0001, '65ms', '180ms', '350ms', 'active', now),
]

export const placeholderServiceCards: ServiceCardData[] = [
  { name: 'api', label: '2,500 req/s', sparkline: generateData(2500, 500) },
  { name: 'web', label: '1,850 req/s', sparkline: generateData(1850, 400) },
  { name: 'db', label: '940 req/s', sparkline: generateData(940, 200) },
  { name: 'worker', label: '250 req/s', sparkline: generateData(250, 80) },
]

export const placeholderServiceCharts: ServiceCharts = {
  requests: [
    { service: 'api', count: 2500, rate: 2500 },
    { service: 'web', count: 1850, rate: 1850 },
    { service: 'db', count: 940, rate: 940 },
    { service: 'worker', count: 250, rate: 250 },
    { service: 'auth', count: 340, rate: 340 },
  ],
  errorRates: [
    { service: 'api', count: 3, rate: 0.12 },
    { service: 'web', count: 1.5, rate: 0.08 },
    { service: 'db', count: 3.2, rate: 0.34 },
    { service: 'worker', count: 0.13, rate: 0.05 },
    { service: 'auth', count: 0.03, rate: 0.01 },
  ],
  latencies: [
    { service: 'api', avg: 45, p50: 35, p95: 120, p99: 250 },
    { service: 'web', avg: 120, p50: 95, p95: 340, p99: 650 },
    { service: 'db', avg: 15, p50: 10, p95: 45, p99: 120 },
    { service: 'worker', avg: 340, p50: 280, p95: 890, p99: 2100 },
    { service: 'auth', avg: 65, p50: 50, p95: 180, p99: 350 },
  ],
}

export const placeholderServiceFilterSections: FilterSectionConfig[] = [
  {
    id: 'health_status',
    title: 'health_status',
    defaultOpen: true,
    items: [
      { id: 'healthy', label: 'Healthy', count: 3, color: '#22c55e' },
      { id: 'degraded', label: 'Degraded', count: 1, color: '#eab308' },
      { id: 'down', label: 'Down', count: 0, color: '#ef4444' },
    ],
  },
  {
    id: 'service_name',
    title: 'service_name',
    items: [
      { id: 'api', label: 'api', count: 2341 },
      { id: 'web', label: 'web', count: 1567 },
      { id: 'db', label: 'db', count: 892 },
      { id: 'worker', label: 'worker', count: 423 },
      { id: 'auth', label: 'auth', count: 340 },
    ],
  },
  {
    id: 'environment',
    title: 'environment',
    items: [
      { id: 'production', label: 'production', count: 5 },
      { id: 'staging', label: 'staging', count: 3 },
      { id: 'development', label: 'development', count: 2 },
    ],
  },
]

/* ── Shared Dropdown Data ── */

export const placeholderTimeRanges = [
  { label: 'Last 15 min', value: 'Last 15 min' },
  { label: 'Last 1 hour', value: 'Last 1 hour' },
  { label: 'Last 6 hours', value: 'Last 6 hours' },
  { label: 'Last 24 hours', value: 'Last 24 hours' },
  { label: 'Last 7 days', value: 'Last 7 days' },
  { label: 'Last 30 days', value: 'Last 30 days' },
  { label: 'Custom', value: 'Custom' },
]

export const placeholderAutoRefreshOptions = [
  { label: 'Off', value: 'Off' },
  { label: '10s', value: '10s' },
  { label: '30s', value: '30s' },
  { label: '1m', value: '1m' },
  { label: '5m', value: '5m' },
]

export const placeholderServices = [
  { label: 'All Services', value: 'All Services' },
  { label: 'web', value: 'web' },
  { label: 'api', value: 'api' },
  { label: 'db', value: 'db' },
  { label: 'worker', value: 'worker' },
]

export const placeholderChartMetrics = [
  { label: 'Count', value: 'count' },
  { label: 'Rate', value: 'rate' },
]

export const placeholderCountRateOptions = [
  { label: 'Count', value: 'count' },
  { label: 'Rate', value: 'rate' },
]

export const placeholderLatencyOptions = [
  { label: 'Avg', value: 'avg' },
  { label: 'P50', value: 'p50' },
  { label: 'P95', value: 'p95' },
  { label: 'P99', value: 'p99' },
]

export const placeholderAnalyticsIngestionOptions = [
  { label: 'Sum', value: 'sum' },
  { label: 'Rate', value: 'rate' },
  { label: 'Volume', value: 'volume' },
]

export const placeholderAnalyticsRateCountOptions = [
  { label: 'Rate', value: 'rate' },
  { label: 'Count', value: 'count' },
]

export const placeholderAnalyticsLatencyOptions = [
  { label: 'P50, P90, P99', value: 'p50_p90_p99' },
  { label: 'P50', value: 'p50' },
  { label: 'P90', value: 'p90' },
  { label: 'P99', value: 'p99' },
  { label: 'Average', value: 'avg' },
]

export const placeholderSortOptions = [
  { label: 'Logs Generated', value: 'logs' },
  { label: 'Errors', value: 'errors' },
  { label: 'Latency', value: 'latency' },
]

export const placeholderRequestsGroupBy = [
  { label: 'nothing', value: 'nothing' },
  { label: 'service', value: 'service' },
  { label: 'level', value: 'level' },
]

export const placeholderErrorsGroupBy = [
  { label: 'nothing', value: 'nothing' },
  { label: 'service', value: 'service' },
  { label: 'level', value: 'level' },
  { label: 'status_code', value: 'status_code' },
]

export const placeholderStatusCodesGroupBy = [
  { label: 'nothing', value: 'nothing' },
  { label: 'service', value: 'service' },
  { label: 'level', value: 'level' },
]