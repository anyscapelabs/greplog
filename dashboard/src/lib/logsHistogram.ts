import type { LogsHistogramData } from '../types/index.ts'

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

function formatBucketLabel(epochMs: number): string {
  return new Date(epochMs).toISOString().slice(11, 16)
}

export function parseLogsHistogram(rows: unknown[][], columns: string[]): LogsHistogramData {
  const bucketIdx = columns.indexOf('bucket')
  const levelIdx = columns.indexOf('level')
  const countIdx = columns.indexOf('cnt')
  if (bucketIdx < 0 || levelIdx < 0 || countIdx < 0) {
    return { buckets: [], levels: [] }
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
    buckets: order.map(formatBucketLabel),
    levels,
  }
}
