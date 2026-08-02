import { test } from 'node:test'
import assert from 'node:assert/strict'
import { parseLogsHistogram } from '../src/lib/logsHistogram.ts'

test('parseLogsHistogram accepts ISO timestamp buckets from query results', () => {
  const result = parseLogsHistogram(
    [
      ['2026-08-03T12:34:00Z', 'info', 3],
      ['2026-08-03T12:34:00Z', 'error', 1],
      ['2026-08-03T12:35:00Z', 'info', 2],
    ],
    ['bucket', 'level', 'cnt'],
  )

  assert.deepEqual(result, {
    buckets: ['12:34', '12:35'],
    levels: [
      { level: 'info', counts: [3, 2] },
      { level: 'error', counts: [1, 0] },
    ],
  })
})

test('parseLogsHistogram accepts microsecond epoch buckets from query results', () => {
  const firstMicros = Date.parse('2026-08-03T12:34:00Z') * 1_000
  const secondMicros = Date.parse('2026-08-03T12:35:00Z') * 1_000

  const result = parseLogsHistogram(
    [
      [firstMicros, 'warn', 4],
      [firstMicros, 'error', 2],
      [secondMicros, 'warn', 1],
    ],
    ['bucket', 'level', 'cnt'],
  )

  assert.deepEqual(result, {
    buckets: ['12:34', '12:35'],
    levels: [
      { level: 'warn', counts: [4, 1] },
      { level: 'error', counts: [2, 0] },
    ],
  })
})

test('parseLogsHistogram skips rows with unknown bucket values instead of fabricating labels', () => {
  const result = parseLogsHistogram(
    [
      ['not-a-timestamp', 'info', 3],
      ['2026-08-03T12:35:00Z', 'error', 1],
    ],
    ['bucket', 'level', 'cnt'],
  )

  assert.deepEqual(result, {
    buckets: ['12:35'],
    levels: [{ level: 'error', counts: [1] }],
  })
})
