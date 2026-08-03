import { test } from 'node:test'
import assert from 'node:assert/strict'
import { compileCheckedToQuery, compileFilterToQuery, type FilterState } from '../src/hooks/useFilterState.ts'

function makeFilters(checked: Record<string, string[]>): FilterState {
  return {
    query: '',
    chips: [],
    services: ['api'],
    timeRange: 'Last 15 min',
    logLevels: [],
    checked,
  }
}

test('log_level checked items compile to a level IN clause', () => {
  assert.equal(compileCheckedToQuery({ log_level: ['error', 'info'] }), "level IN ('error','info')")
})

test('response_status buckets compile to status ranges', () => {
  assert.equal(
    compileCheckedToQuery({ response_status: ['success', 'server_error'] }),
    '(line < 300 OR line >= 500)',
  )
})

test('status_code compiles to numeric line IN literals', () => {
  assert.equal(compileCheckedToQuery({ status_code: ['200', '500'] }), 'line IN (200,500)')
})

test('error_type and service_name quote and escape single quotes', () => {
  assert.equal(compileCheckedToQuery({ error_type: ["o'ne"] }), "exception_type IN ('o''ne')")
})

test('empty and client-side-only sections compile to nothing', () => {
  assert.equal(compileCheckedToQuery({}), '')
  assert.equal(compileCheckedToQuery({ health_status: ['healthy'] }), '')
  assert.equal(compileCheckedToQuery({ log_level: [] }), '')
})

test('full predicate includes every checked section', () => {
  const pred = compileFilterToQuery(makeFilters({ log_level: ['error'], service_name: ['api'] }))
  assert.match(pred, /level IN \('error'\)/)
  assert.match(pred, /service IN \('api'\)/)
})

test('level facet predicate excludes the level selection but keeps other filters', () => {
  const pred = compileFilterToQuery(makeFilters({ log_level: ['error'], service_name: ['api'] }), undefined, {
    excludeCheckedSections: ['log_level'],
    excludeLogLevels: true,
  })
  assert.match(pred, /service IN \('api'\)/)
  assert.doesNotMatch(pred, /level IN/)
})

test('service facet predicate excludes the service selection but keeps the level selection', () => {
  const pred = compileFilterToQuery(makeFilters({ log_level: ['error'], service_name: ['api'] }), undefined, {
    excludeCheckedSections: ['service_name'],
    excludeServices: true,
  })
  assert.match(pred, /level IN \('error'\)/)
  assert.doesNotMatch(pred, /service IN/)
})

test('status facet predicate excludes status sections but keeps other dimensions', () => {
  const pred = compileFilterToQuery(
    makeFilters({ log_level: ['error'], status_code: ['500'], response_status: ['server_error'] }),
    undefined,
    { excludeCheckedSections: ['status_code', 'response_status'] },
  )
  assert.match(pred, /level IN \('error'\)/)
  assert.doesNotMatch(pred, /line IN/)
  assert.doesNotMatch(pred, /line >= 500/)
})

test('facet predicate excludes every facet selection (page usage)', () => {
  const pred = compileFilterToQuery(
    makeFilters({ log_level: ['error'], service_name: ['api'], status_code: ['500'], response_status: ['server_error'] }),
    undefined,
    {
      excludeCheckedSections: ['log_level', 'service_name', 'status_code', 'response_status'],
      excludeServices: true,
      excludeLogLevels: true,
    },
  )
  assert.doesNotMatch(pred, /level IN/)
  assert.doesNotMatch(pred, /service IN/)
  assert.doesNotMatch(pred, /line IN/)
  assert.doesNotMatch(pred, /line >= 500/)
  assert.match(pred, /timestamp > to_timestamp_micros/)
})
