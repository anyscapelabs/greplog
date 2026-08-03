import type { LogsHistogramData, LogsHistogramGranularity } from '../types/index.ts'

function parseBucketEpochMs(raw: unknown): number | null {
  if (typeof raw === 'bigint') {
    if (raw <= 0n) return null
    return Number(raw / 1000n)
  }

  if (typeof raw === 'number') {
    if (!Number.isFinite(raw) || raw <= 0) return null
    if (raw >= 1_000_000_000_000_000) return Math.trunc(raw / 1_000)
    if (raw >= 1_000_000_000_000) return Math.trunc(raw)
    if (raw >= 1_000_000_000) return Math.trunc(raw * 1_000)
    return null
  }

  if (raw instanceof Date) {
    const ms = raw.getTime()
    return Number.isFinite(ms) && ms > 0 ? ms : null
  }

  const text = String(raw ?? '').trim()
  if (!text) return null

  if (/^\d+$/.test(text)) {
    const value = Number(text)
    if (!Number.isFinite(value) || value <= 0) return null
    if (value >= 1_000_000_000_000_000) return Math.trunc(value / 1_000)
    if (value >= 1_000_000_000_000) return Math.trunc(value)
    if (value >= 1_000_000_000) return Math.trunc(value * 1_000)
    return null
  }

  const parsed = Date.parse(text)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null
}

function pad2(n: number): string {
  return String(n).padStart(2, '0')
}

// Bucket label format depends on granularity: minute buckets only need a
// time-of-day label (the whole range fits in a few hours), but hour/day
// buckets span multiple calendar days, so the date must be shown or the
// x-axis becomes ambiguous (e.g. every "12:00" tick looks identical across
// a 7-day range).
function formatBucketLabel(epochMs: number, granularity: LogsHistogramGranularity): string {
  const d = new Date(epochMs)
  const month = pad2(d.getUTCMonth() + 1)
  const day = pad2(d.getUTCDate())
  const hours = pad2(d.getUTCHours())
  const minutes = pad2(d.getUTCMinutes())

  if (granularity === 'day') return `${month}/${day}`
  if (granularity === 'hour') return `${month}/${day} ${hours}:00`
  return `${hours}:${minutes}`
}

export function parseLogsHistogram(rows: unknown[][], columns: string[], granularity: LogsHistogramGranularity = 'minute'): LogsHistogramData {
  const bucketIdx = columns.indexOf('bucket')
  const levelIdx = columns.indexOf('level')
  const countIdx = columns.indexOf('cnt')
  if (bucketIdx < 0 || levelIdx < 0 || countIdx < 0) {
    return { buckets: [], levels: [], granularity }
  }

  const order: number[] = []
  const bucketIndexOf = new Map<number, number>()
  const levelCells = new Map<string, Map<number, number>>()

  for (const row of rows) {
    const bucketMs = parseBucketEpochMs(row[bucketIdx])
    const level = String(row[levelIdx] ?? '').trim()
    const count = Number(row[countIdx] ?? 0)
    if (bucketMs === null || !level || !Number.isFinite(count) || count < 0) continue

    let bucketIndex = bucketIndexOf.get(bucketMs)
    if (bucketIndex === undefined) {
      bucketIndex = order.length
      bucketIndexOf.set(bucketMs, bucketIndex)
      order.push(bucketMs)
    }

    let cells = levelCells.get(level)
    if (cells === undefined) {
      cells = new Map()
      levelCells.set(level, cells)
    }
    cells.set(bucketIndex, (cells.get(bucketIndex) ?? 0) + count)
  }

  const levels = Array.from(levelCells.entries()).map(([level, cells]) => {
    const counts = new Array(order.length).fill(0)
    for (const [bucketIndex, count] of cells) {
      counts[bucketIndex] = count
    }
    return { level, counts }
  })

  return {
    buckets: order.map((epochMs) => formatBucketLabel(epochMs, granularity)),
    levels,
    granularity,
  }
}
