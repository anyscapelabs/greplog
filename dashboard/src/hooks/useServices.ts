import { useQuery } from '@tanstack/react-query'
import { useCallback, useRef } from 'react'
import type { ServicesPageProps, ServiceEntry, ServiceCharts } from '../types/index.ts'
import { classifyHealth } from '../types/index.ts'
import { fetchDetect, postQuery, type DetectEntry } from './api.ts'
import {
  placeholderServiceFilterSections,
  placeholderTimeRanges,
  placeholderAutoRefreshOptions,
  placeholderCountRateOptions,
  placeholderLatencyOptions,
} from './placeholder-data.ts'
import { useAgent } from '../context/AgentContext.tsx'
import { TIME_RANGE_NS } from './useFilterState.ts'
import { buildAvgLatencyByServiceSql, parseAvgLatencyByService } from '../lib/httpQueries.ts'

const DEFAULT_WINDOW_NS = 30 * 60 * 1_000_000_000 // 30 minutes in nanoseconds
function unionServices(detect: DetectEntry[], activeServices: string[], healthMap: Map<string, { errorRate: number; eventCount: number; firstSeen: number }>): ServiceEntry[] {
  const detectedNames = new Set(
    detect.map((d) => d.service_name).filter((n): n is string => n !== null),
  )
  const activeNames = new Set(activeServices)

  const seen = new Set<string>()
  const result: ServiceEntry[] = []

  function entry(name: string, status: 'active' | 'detected_only'): ServiceEntry {
    const h = healthMap.get(name)
    const errorRate = h?.errorRate ?? 0
    const eventCount = h?.eventCount ?? 0
    const firstSeenMicros = h?.firstSeen ?? 0
    const firstSeenDate = firstSeenMicros > 0 ? new Date(firstSeenMicros / 1_000).toISOString() : undefined
    return {
      id: name,
      name,
      status,
      health: classifyHealth(errorRate, eventCount),
      errorRate,
      eventCount,
      uptime: '',
      requests: '0',
      avgLatency: '0ms',
      p95: '0ms',
      p99: '0ms',
      lastSeen: '',
      firstSeen: firstSeenDate,
    }
  }

  for (const name of detectedNames) {
    seen.add(name)
    result.push(entry(name, activeNames.has(name) ? 'active' : 'detected_only'))
  }

  for (const name of activeNames) {
    if (!seen.has(name)) {
      result.push(entry(name, 'active'))
    }
  }

  return result
}

export function useServices(timeRange?: string): ServicesPageProps {
  const { connected } = useAgent()
  const userInitiatedRef = useRef(false)

  const detectQuery = useQuery({
    queryKey: ['services', 'detect'],
    queryFn: async () => {
      if (!connected) return []
      return (await fetchDetect()) ?? []
    },
    enabled: connected,
  })

  const windowNs = timeRange && TIME_RANGE_NS[timeRange] ? TIME_RANGE_NS[timeRange] : DEFAULT_WINDOW_NS
  const logQuery = useQuery({
    queryKey: ['services', 'log-scan', windowNs],
    queryFn: async () => {
      if (!connected) return { services: [] }
      const userInitiated = userInitiatedRef.current
      userInitiatedRef.current = false
      const nowMicros = Date.now() * 1_000
      const result = await postQuery(
        `SELECT DISTINCT service FROM logs WHERE timestamp > ${nowMicros - windowNs / 1_000}`,
        { userInitiated },
      )
      if (!result) return { services: [] }
      return {
        services: result.rows.map((r) => String(r[0] ?? '')),
      }
    },
    enabled: connected,
  })

  const healthQuery = useQuery({
    queryKey: ['services', 'health', windowNs],
    queryFn: async () => {
      if (!connected) return []
      const userInitiated = userInitiatedRef.current
      userInitiatedRef.current = false
      const nowMicros = Date.now() * 1_000
      const result = await postQuery(
        `SELECT service, count(*) AS total, count(*) FILTER (WHERE level = 'error') AS errors, CAST(count(*) FILTER (WHERE level = 'error') AS DOUBLE) / CAST(count(*) AS DOUBLE) AS error_rate, MIN(timestamp) AS first_seen FROM logs WHERE timestamp > ${nowMicros - windowNs / 1_000} GROUP BY service`,
        { userInitiated },
      )
      if (!result) return []
      const errIdx = result.columns.indexOf('errors')
      const totalIdx = result.columns.indexOf('total')
      const svcIdx = result.columns.indexOf('service')
      const fsIdx = result.columns.indexOf('first_seen')
      const rateIdx = result.columns.indexOf('error_rate')
      if (svcIdx < 0 || totalIdx < 0 || errIdx < 0) return []
      return result.rows.map((r) => ({
        service: String(r[svcIdx] ?? ''),
        errorRate: rateIdx >= 0 ? Number(r[rateIdx] ?? 0) : 0,
        eventCount: Number(r[totalIdx] ?? 0),
        firstSeen: fsIdx >= 0 ? Number(r[fsIdx] ?? 0) : 0,
      }))
    },
    enabled: connected,
  })

  const sparklineQuery = useQuery({
    queryKey: ['services', 'sparkline', windowNs],
    queryFn: async () => {
      if (!connected) return new Map<string, number[]>()
      const userInitiated = userInitiatedRef.current
      userInitiatedRef.current = false
      const nowMicros = Date.now() * 1_000
      const result = await postQuery(
        `SELECT service, FLOOR(timestamp / 60000000) AS bucket, count(*) AS cnt FROM logs WHERE timestamp > ${nowMicros - windowNs / 1_000} GROUP BY service, bucket ORDER BY service, bucket`,
        { userInitiated },
      )
      if (!result) return new Map()
      const svcIdx = result.columns.indexOf('service')
      const bucketIdx = result.columns.indexOf('bucket')
      const cntIdx = result.columns.indexOf('cnt')
      if (svcIdx < 0 || bucketIdx < 0 || cntIdx < 0) return new Map()
      const byService = new Map<string, { bucket: number; cnt: number }[]>()
      for (const row of result.rows) {
        const svc = String(row[svcIdx] ?? '')
        if (!svc) continue
        const bucket = Number(row[bucketIdx] ?? 0)
        const cnt = Number(row[cntIdx] ?? 0)
        if (!byService.has(svc)) byService.set(svc, [])
        byService.get(svc)!.push({ bucket, cnt })
      }
      const sparklines = new Map<string, number[]>()
      for (const [svc, buckets] of byService) {
        buckets.sort((a, b) => a.bucket - b.bucket)
        sparklines.set(svc, buckets.map((b) => b.cnt))
      }
      return sparklines
    },
    enabled: connected,
  })

  // Per-service latency (avg/p50/p95/p99) from the dual-source HTTP population
  // (spans UNION ALL logs.attributes) — the same implementation the Analytics
  // page uses, sharing httpArmPredicates. The time window is the only
  // predicate, matching the other services queries.
  const latencyQuery = useQuery({
    queryKey: ['services', 'latency', windowNs],
    queryFn: async () => {
      if (!connected) return []
      const userInitiated = userInitiatedRef.current
      userInitiatedRef.current = false
      const nowMicros = Date.now() * 1_000
      const dual = buildAvgLatencyByServiceSql(`timestamp > ${nowMicros - windowNs / 1_000}`)
      if (!dual.sql) return []
      const result = await postQuery(dual.sql, { userInitiated })
      if (!result) return []
      return parseAvgLatencyByService(result)
    },
    enabled: connected,
  })

  const detectedNames = detectQuery.data ?? []
  const activeServices = logQuery.data?.services ?? []
  const healthRows = healthQuery.data ?? []
  const sparklineMap = sparklineQuery.data ?? new Map()

  const healthMap = new Map<string, { errorRate: number; eventCount: number; firstSeen: number }>(healthRows.map((h) => [h.service, { errorRate: h.errorRate, eventCount: h.eventCount, firstSeen: h.firstSeen }]))

  const services = unionServices(detectedNames, activeServices, healthMap)

  const serviceCards = services.map((s) => ({
    name: s.name,
    label: `${(s.errorRate * 100).toFixed(1)}% err — ${s.eventCount} events`,
    sparkline: sparklineMap.get(s.name) ?? [],
  }))

  function refetchServices() {
    void detectQuery.refetch()
    void logQuery.refetch()
    void healthQuery.refetch()
    void sparklineQuery.refetch()
    void latencyQuery.refetch()
  }

  const manualRefetch = useCallback(() => {
    userInitiatedRef.current = true
    refetchServices()
  }, [userInitiatedRef, refetchServices])

  const charts: ServiceCharts = {
    requests: services.map((s) => ({ service: s.name, count: s.eventCount, rate: s.eventCount })),
    errorRates: services.map((s) => ({ service: s.name, count: Math.round(s.errorRate * s.eventCount), rate: s.errorRate })),
    latencies: latencyQuery.data ?? [],
  }

  return {
    services,
    totalRows: services.length,
    querySeconds: 0,
    filterSections: placeholderServiceFilterSections,
    serviceCards,
    charts,
    isWaiting: !connected,
    timeRanges: placeholderTimeRanges,
    autoRefreshOptions: placeholderAutoRefreshOptions,
    countRateOptions: placeholderCountRateOptions,
    latencyOptions: placeholderLatencyOptions,
    onViewService: undefined,
    refetch: refetchServices,
    manualRefetch,
  }
}