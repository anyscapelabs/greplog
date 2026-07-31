import { useQuery } from '@tanstack/react-query'
import type { AnalyticsPageProps, PieSlice } from '../types/index.ts'
import { postQuery, type QueryResult } from './api.ts'
import {
  placeholderAnalyticsMetrics,
  placeholderTimeRanges,
  placeholderServices,
  placeholderAutoRefreshOptions,
  placeholderAnalyticsIngestionOptions,
  placeholderAnalyticsRateCountOptions,
  placeholderAnalyticsLatencyOptions,
  placeholderSortOptions,
} from './placeholder-data.ts'
import { useAgent } from '../context/AgentContext.tsx'

const EMPTY_RESULT = {
  metrics: placeholderAnalyticsMetrics,
  ingestionTimeseries: [] as AnalyticsPageProps['ingestionTimeseries'],
  errorRateTimeseries: [] as AnalyticsPageProps['errorRateTimeseries'],
  latencyData: { p50: [], p90: [], p99: [] } as AnalyticsPageProps['latencyData'],
  serviceHealthData: [] as AnalyticsPageProps['serviceHealthData'],
  statusCodeDistribution: [] as AnalyticsPageProps['statusCodeDistribution'],
  noisyServices: [] as AnalyticsPageProps['noisyServices'],
  severityDistribution: [] as AnalyticsPageProps['severityDistribution'],
  systemMetrics: { cpu: [], memory: [], diskIO: [], network: [] } as AnalyticsPageProps['systemMetrics'],
  avgResponseTimes: [] as AnalyticsPageProps['avgResponseTimes'],
  isWaiting: true,
}

function parseCountTimeseries(result: QueryResult): AnalyticsPageProps['ingestionTimeseries'] {
  const idx = (name: string) => result.columns.indexOf(name)
  const tsIdx = idx('date')
  const cntIdx = idx('cnt')
  if (tsIdx < 0 || cntIdx < 0) return []
  return result.rows.map((r) => ({
    timestamp: String(r[tsIdx] ?? ''),
    value: Number(r[cntIdx] ?? 0),
  }))
}

function parseServiceHealth(result: QueryResult): AnalyticsPageProps['serviceHealthData'] {
  const idx = (name: string) => result.columns.indexOf(name)
  const svcIdx = idx('service')
  const healthyIdx = idx('healthy')
  const errIdx = idx('errors')
  if (svcIdx < 0 || healthyIdx < 0 || errIdx < 0) return []
  return result.rows.map((r) => ({
    name: String(r[svcIdx] ?? ''),
    healthy: Number(r[healthyIdx] ?? 0),
    degraded: 0,
    down: Number(r[errIdx] ?? 0),
  }))
}

function parseNoisyServices(result: QueryResult): AnalyticsPageProps['noisyServices'] {
  const idx = (name: string) => result.columns.indexOf(name)
  const svcIdx = idx('service')
  const cntIdx = idx('cnt')
  if (svcIdx < 0) return []
  return result.rows.map((r) => ({
    name: String(r[svcIdx] ?? ''),
    count: Number(r[cntIdx] ?? 0),
  }))
}

function parseSeverityDistribution(result: QueryResult): AnalyticsPageProps['severityDistribution'] {
  const idx = (name: string) => result.columns.indexOf(name)
  const lvlIdx = idx('level')
  const cntIdx = idx('cnt')
  if (lvlIdx < 0) return []
  const colorMap: Record<string, string> = {
    info: '#3b82f6',
    warn: '#f59e0b',
    error: '#ef4444',
    debug: '#8b5cf6',
    critical: '#dc2626',
  }
  return result.rows.map((r) => ({
    name: String(r[lvlIdx] ?? ''),
    value: Number(r[cntIdx] ?? 0),
    color: colorMap[String(r[lvlIdx] ?? '').toLowerCase()] ?? '#6b7280',
  }))
}

/**
 * Parse a single-row summary result from the server-side metrics query.
 *
 * Query shape (one row):
 *   total_events       BIGINT
 *   total_errors       BIGINT
 *   active_services    BIGINT   (COUNT DISTINCT service)
 *   unhealthy_services BIGINT   (COUNT DISTINCT service WHERE error_rate > 5%)
 *
 * All four values are computed by the query engine (DataFusion GROUP BY /
 * aggregate), not derived client-side from the per-service rows.
 */
function parseSummaryMetrics(result: QueryResult): {
  totalEvents: number
  totalErrors: number
  activeServices: number
  unhealthyServices: number
} {
  const idx = (name: string) => result.columns.indexOf(name)
  const row = result.rows[0] ?? []
  return {
    totalEvents: Number(row[idx('total_events')] ?? 0),
    totalErrors: Number(row[idx('total_errors')] ?? 0),
    activeServices: Number(row[idx('active_services')] ?? 0),
    unhealthyServices: Number(row[idx('unhealthy_services')] ?? 0),
  }
}

export function useAnalytics(whereClause?: string): AnalyticsPageProps {
  const { connected } = useAgent()

  const query = useQuery({
    queryKey: whereClause ? ['analytics', whereClause] : ['analytics'],
    queryFn: async () => {
      if (!connected) return EMPTY_RESULT
      const w = whereClause ?? ''
      // Compose the optional user-provided WHERE clause as an AND predicate
      // for queries that already have their own WHERE clause.
      const andClause = w ? ` AND (${w.replace(/^WHERE\s+/i, '')})` : ''

      // ── Five parallel queries, all aggregation done server-side ──────────

      // 1. Summary metrics — one row, four computed aggregates.
      //    "unhealthy" = services where errors > 5% of their events.
      //    This is a subquery because DataFusion doesn't support HAVING on
      //    a FILTER aggregate in a scalar position directly.
      const summarysql = `
        SELECT
          SUM(total)          AS total_events,
          SUM(errors)         AS total_errors,
          COUNT(*)            AS active_services,
          COUNT(*) FILTER (WHERE total > 0 AND errors * 1.0 / total > 0.05) AS unhealthy_services
        FROM (
          SELECT
            service,
            count(*)                                                       AS total,
            count(*) FILTER (WHERE level IN ('error','critical','fatal'))  AS errors
          FROM logs ${w}
          GROUP BY service
        )`

      // 2. Ingestion volume timeseries (count per date bucket)
      const ingestionSql = `SELECT date, count(*) AS cnt FROM logs ${w} GROUP BY date ORDER BY date`

      // 3. Error count timeseries (count per date bucket, errors only)
      const errorRateSql = `SELECT date, count(*) AS cnt FROM logs WHERE level IN ('error','critical','fatal')${andClause} GROUP BY date ORDER BY date`

      // 4. Per-service health breakdown for the service-health bar chart
      const healthSql = `
        SELECT
          service,
          count(*) - count(*) FILTER (WHERE level IN ('error','critical','fatal'))  AS healthy,
          count(*) FILTER (WHERE level IN ('error','critical','fatal'))             AS errors
        FROM logs ${w}
        GROUP BY service`

      // 5. Noisy services (top-5 by event count)
      const noisySql = `SELECT service, count(*) AS cnt FROM logs ${w} GROUP BY service ORDER BY cnt DESC LIMIT 5`

      // 6. Severity distribution (count per level)
      const severitySql = `SELECT level, count(*) AS cnt FROM logs ${w} GROUP BY level ORDER BY cnt DESC`

      // ── Spans queries (3) ─────────────────────────────────────────────────
      // These are best-effort — if the predicate references columns that don't
      // exist in the spans table (e.g. `level`), postQuery returns null and
      // the charts fall back to ChartEmptyState.
      const spansAnd = w ? ` WHERE ${w.replace(/^WHERE\s+/i, '')}` : ''
      const latencySql = `SELECT approx_percentile_cont(latency_ms, 0.50) AS p50, approx_percentile_cont(latency_ms, 0.90) AS p90, approx_percentile_cont(latency_ms, 0.99) AS p99 FROM spans${spansAnd}`
      const statusCodeSql = `SELECT status_code, count(*) AS cnt FROM spans${spansAnd} GROUP BY status_code ORDER BY cnt DESC`
      const avgRespSql = `SELECT service, avg(latency_ms) AS avg_ms FROM spans${spansAnd} GROUP BY service ORDER BY avg_ms DESC`

      const [summary, ingestion, errorRate, health, noisy, severity, latencyResult, statusCodeResult, avgRespResult] = await Promise.all([
        postQuery(summarysql),
        postQuery(ingestionSql),
        postQuery(errorRateSql),
        postQuery(healthSql),
        postQuery(noisySql),
        postQuery(severitySql),
        postQuery(latencySql),
        postQuery(statusCodeSql),
        postQuery(avgRespSql),
      ])

      const { totalEvents, totalErrors, activeServices, unhealthyServices } =
        summary ? parseSummaryMetrics(summary) : { totalEvents: 0, totalErrors: 0, activeServices: 0, unhealthyServices: 0 }

      // ── Parse spans results ─────────────────────────────────────────────
      function parseLatency(result: QueryResult | null): { p50: number[]; p90: number[]; p99: number[] } {
        if (!result || !result.rows[0]) return { p50: [], p90: [], p99: [] }
        const idx = (name: string) => result.columns.indexOf(name)
        return {
          p50: [Number(result.rows[0][idx('p50')] ?? 0)],
          p90: [Number(result.rows[0][idx('p90')] ?? 0)],
          p99: [Number(result.rows[0][idx('p99')] ?? 0)],
        }
      }

      function parseStatusCodes(result: QueryResult | null): PieSlice[] {
        if (!result) return []
        const codeIdx = result.columns.indexOf('status_code')
        const cntIdx = result.columns.indexOf('cnt')
        if (codeIdx < 0 || cntIdx < 0) return []
        const colorMap: Record<number, string> = {
          2: '#22c55e', 3: '#3b82f6', 4: '#f59e0b', 5: '#ef4444',
        }
        return result.rows.map((r) => {
          const code = Number(r[codeIdx] ?? 0)
          const prefix = Math.floor(code / 100)
          return {
            name: String(code),
            value: Number(r[cntIdx] ?? 0),
            color: colorMap[prefix] ?? '#6b7280',
          }
        })
      }

      function parseAvgResponseTime(result: QueryResult | null): { service: string; ms: number }[] {
        if (!result) return []
        const svcIdx = result.columns.indexOf('service')
        const msIdx = result.columns.indexOf('avg_ms')
        if (svcIdx < 0 || msIdx < 0) return []
        return result.rows.map((r) => ({
          service: String(r[svcIdx] ?? ''),
          ms: Math.round(Number(r[msIdx] ?? 0) * 100) / 100,
        }))
      }

      const overallErrorRate = totalEvents > 0 ? totalErrors / totalEvents : 0

      const ingestionTs = ingestion ? parseCountTimeseries(ingestion) : []
      const errorTs = errorRate ? parseCountTimeseries(errorRate) : []
      const sparklineData = ingestionTs.map((p) => p.value)

      const metrics = [
        { title: 'Error rate', value: `${(overallErrorRate * 100).toFixed(2)}%`, color: '#dc2626', rgb: '220, 38, 38', sparkline: errorTs.map((p) => p.value) },
        { title: 'Active services', value: String(activeServices), color: '#2563eb', rgb: '37, 99, 235', sparkline: [] as number[] },
        { title: 'Unhealthy services', value: String(unhealthyServices), color: '#dc2626', rgb: '220, 38, 38', sparkline: [] as number[] },
        { title: 'Total events', value: totalEvents >= 1000 ? `${(totalEvents / 1000).toFixed(1)}k` : String(totalEvents), color: '#16a34a', rgb: '22, 163, 74', sparkline: sparklineData },
        { title: 'Requests', value: totalEvents >= 1000 ? `${(totalEvents / 1000).toFixed(1)}k` : String(totalEvents), color: '#3b82f6', rgb: '59, 130, 246', sparkline: sparklineData },
        { title: 'P.95 latency', value: 'N/A', color: '#d97706', rgb: '217, 119, 6', sparkline: [] as number[] },
      ]

      const latencyData = latencyResult ? parseLatency(latencyResult) : { p50: [], p90: [], p99: [] }
      const statusCodeDistribution = statusCodeResult ? parseStatusCodes(statusCodeResult) : []
      const avgResponseTimes = avgRespResult ? parseAvgResponseTime(avgRespResult) : []

      // Update "Requests" metric to use real event count (same as total events for now)
      metrics[4] = { ...metrics[4], value: totalEvents >= 1000 ? `${(totalEvents / 1000).toFixed(1)}k` : String(totalEvents) }
      // Update "P.95 latency" metric using real p90 latency data
      if (latencyData.p90.length > 0) {
        metrics[5] = { ...metrics[5], value: `${latencyData.p90[0].toFixed(0)}ms` }
      }

      return {
        metrics,
        ingestionTimeseries: ingestionTs,
        errorRateTimeseries: errorTs,
        latencyData,
        serviceHealthData: health ? parseServiceHealth(health) : [],
        statusCodeDistribution,
        noisyServices: noisy ? parseNoisyServices(noisy) : [],
        severityDistribution: severity ? parseSeverityDistribution(severity) : [],
        systemMetrics: { cpu: [], memory: [], diskIO: [], network: [] },
        avgResponseTimes,
        isWaiting: false,
      }
    },
    enabled: connected,
  })

  const data = query.data ?? EMPTY_RESULT

  return {
    ...data,
    timeRanges: placeholderTimeRanges,
    services: placeholderServices,
    autoRefreshOptions: placeholderAutoRefreshOptions,
    ingestionOptions: placeholderAnalyticsIngestionOptions,
    rateCountOptions: placeholderAnalyticsRateCountOptions,
    latencyOptions: placeholderAnalyticsLatencyOptions,
    sortOptions: placeholderSortOptions,
    ingestionMetric: 'sum',
    onIngestionMetricChange: () => {},
    errorRateMetric: 'rate',
    onErrorRateMetricChange: () => {},
    latencyView: 'p50_p90_p99',
    onLatencyViewChange: () => {},
    statusCodeMetric: 'rate',
    onStatusCodeMetricChange: () => {},
    noisySort: 'logs',
    onNoisySortChange: () => {},
    refetch: query.refetch,
  }
}