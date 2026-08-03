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
// time-of-day label (the whole range fits in a few hours), but hour/12-hour/day
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
  if (granularity === '12-hour') return `${month}/${day} ${hours}:00`
  return `${hours}:${minutes}`
}

// For 12-hour granularity, we receive hourly data from the query and aggregate
// pairs of hours (0-11 and 12-23) into two buckets per day.
function aggregate12HourFromHourly(parsed: {
  buckets: string[]
  levels: Array<{ level: string; counts: number[] }>
}): { buckets: string[]; levels: Array<{ level: string; counts: number[] }> } {
  const data = parsed
  if (data.buckets.length === 0) return data

  // Group consecutive hourly buckets into 12-hour chunks
  // The first chunk is hours 0-11, the second is 12-23
  const parseHourLabel = (label: string): { month: number; day: number; hour: number } | null => {
    const match = label.match(/(\d{2})\/(\d{2})\s+(\d{2}):00/)
    if (!match) return null
    return {
      month: parseInt(match[1], 10),
      day: parseInt(match[2], 10),
      hour: parseInt(match[3], 10),
    }
  }

  const aggregatedBuckets: string[] = []
  const aggregatedCountsByLevel = new Map<string, number[]>()

  // Initialize level maps
  for (const level of data.levels) {
    aggregatedCountsByLevel.set(level.level, [])
  }

  let currentDayKey: string | null = null
  let currentPeriod: 'morning' | 'afternoon' = 'morning' // 0-11 or 12-23

  for (let i = 0; i < data.buckets.length; i++) {
    const parsed = parseHourLabel(data.buckets[i])
    if (!parsed) continue

    const dayKey = `${parsed.month}/${parsed.day}`
    const period = parsed.hour < 12 ? 'morning' : 'afternoon'
    const bucketLabel = `${parsed.month}/${parsed.day} ${period === 'morning' ? '00' : '12'}:00`

    // When we transition to a new period or day, save the aggregated bucket
    const isNewPeriod = currentDayKey !== dayKey || currentPeriod !== period
    if (isNewPeriod && aggregatedBuckets.length > 0 && currentDayKey !== null) {
      // We're moving to a new bucket, so finalize the previous one
      aggregatedBuckets.push(bucketLabel)
    }

    if (isNewPeriod) {
      currentDayKey = dayKey
      currentPeriod = period
      if (aggregatedBuckets.length === 0) {
        aggregatedBuckets.push(bucketLabel)
      }

      // Reset counts for new aggregation
      for (const level of data.levels) {
        if (!aggregatedCountsByLevel.has(level.level)) {
          aggregatedCountsByLevel.set(level.level, [])
        }
        const counts = aggregatedCountsByLevel.get(level.level)!
        if (counts.length < aggregatedBuckets.length) {
          counts.push(0)
        }
      }
    }

    // Add this hour's counts to the current aggregated bucket
    const aggregatedIdx = aggregatedBuckets.length - 1
    for (const level of data.levels) {
      const counts = aggregatedCountsByLevel.get(level.level)!
      if (counts[aggregatedIdx] === undefined) {
        counts[aggregatedIdx] = 0
      }
      counts[aggregatedIdx] += level.counts[i] ?? 0
    }
  }

  // Build the result
  const aggregatedLevels = data.levels.map((level) => ({
    level: level.level,
    counts: aggregatedCountsByLevel.get(level.level) || [],
  }))

  return {
    buckets: aggregatedBuckets,
    levels: aggregatedLevels,
  }
}

// Fill missing buckets across the full date range with zeros.
// For 12-hour or daily granularity, this ensures dates with no data are shown
// with zero counts instead of being skipped, so users see the full range.
function fillFullDateRange(
  data: { buckets: string[]; levels: Array<{ level: string; counts: number[] }> },
  granularity: LogsHistogramGranularity,
): { buckets: string[]; levels: Array<{ level: string; counts: number[] }> } {
  if (data.buckets.length === 0) return data

  // Only fill for 12-hour and day granularities, not minute or hour
  if (granularity === 'minute' || granularity === 'hour') return data

  // Calculate step size based on granularity
  const stepMs = granularity === '12-hour' ? 12 * 60 * 60 * 1000 : 24 * 60 * 60 * 1000

  // Parse dates from labels (format: "MM/DD" or "MM/DD HH:00")
  const parseLabel = (label: string): Date | null => {
    const match = label.match(/(\d{2})\/(\d{2})(?:\s+(\d{2}):00)?/)
    if (!match) return null
    const month = parseInt(match[1], 10) - 1
    const day = parseInt(match[2], 10)
    const hour = match[3] ? parseInt(match[3], 10) : 0
    const now = new Date()
    return new Date(Date.UTC(now.getUTCFullYear(), month, day, hour, 0, 0, 0))
  }

  const firstDate = parseLabel(data.buckets[0])
  const lastDate = parseLabel(data.buckets[data.buckets.length - 1])

  if (!firstDate || !lastDate) return data

  // Generate full range of bucket timestamps
  const filledBuckets: string[] = []
  const filledOrder = new Map<string, number>()

  for (let current = new Date(firstDate); current <= lastDate; current = new Date(current.getTime() + stepMs)) {
    const label = formatBucketLabel(current.getTime(), granularity)
    if (!filledOrder.has(label)) {
      filledBuckets.push(label)
      filledOrder.set(label, filledBuckets.length - 1)
    }
  }

  // Map old bucket labels to new indices
  const oldLabelToNewIdx = new Map<string, number>()
  for (const oldLabel of data.buckets) {
    const newIdx = filledOrder.get(oldLabel)
    if (newIdx !== undefined) {
      oldLabelToNewIdx.set(oldLabel, newIdx)
    }
  }

  // Rebuild levels with extended counts array
  const filledLevels = data.levels.map((level) => {
    const newCounts = new Array(filledBuckets.length).fill(0)
    for (let i = 0; i < data.buckets.length; i++) {
      const oldLabel = data.buckets[i]
      const newIdx = oldLabelToNewIdx.get(oldLabel)
      if (newIdx !== undefined && level.counts[i] !== undefined) {
        newCounts[newIdx] = level.counts[i]
      }
    }
    return { ...level, counts: newCounts }
  })

  return {
    buckets: filledBuckets,
    levels: filledLevels,
  }
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

  // Format buckets with the appropriate granularity label
  // For 12-hour, we'll first format as hourly, then aggregate
  let parsed = {
    buckets: order.map((epochMs) => formatBucketLabel(epochMs, granularity === '12-hour' ? 'hour' : granularity)),
    levels,
  }

  // For 12-hour granularity, aggregate hourly data into 12-hour chunks
  if (granularity === '12-hour') {
    parsed = aggregate12HourFromHourly(parsed)
  }

  // Fill missing dates across the full range for multi-day granularities
  const filled = fillFullDateRange(parsed, granularity)

  return {
    buckets: filled.buckets,
    levels: filled.levels,
    granularity,
  }
}
