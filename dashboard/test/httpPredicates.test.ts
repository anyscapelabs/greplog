import { test } from 'node:test'
import assert from 'node:assert/strict'
import { splitPredicateClauses, splitQuotedList, httpArmPredicates } from '../src/lib/httpPredicates.ts'

test('splitPredicateClauses splits on top-level AND only', () => {
  assert.deepEqual(splitPredicateClauses("WHERE correlation_id = 'abc' AND level = 'error'"), [
    "correlation_id = 'abc'",
    "level = 'error'",
  ])
})

test('splitPredicateClauses keeps AND inside quoted literals', () => {
  assert.deepEqual(splitPredicateClauses("WHERE message LIKE '%a AND b%'"), ["message LIKE '%a AND b%'"])
})

test('splitPredicateClauses handles doubled-quote escaping', () => {
  assert.deepEqual(splitPredicateClauses("WHERE correlation_id = 'o''ne AND two'"), ["correlation_id = 'o''ne AND two'"])
})

test('splitPredicateClauses strips WHERE and returns [] for empty input', () => {
  assert.deepEqual(splitPredicateClauses(''), [])
  assert.deepEqual(splitPredicateClauses('WHERE '), [])
})

test('splitQuotedList returns bare literal strings', () => {
  assert.deepEqual(splitQuotedList("'error', 'warn'"), ['error', 'warn'])
  assert.deepEqual(splitQuotedList(''), [])
})

test('correlation_id regex does not swallow a following predicate', () => {
  // Reviewer regression: greedy `.*` in the correlation_id matcher must not
  // consume `AND level IN ('error')`. splitPredicateClauses isolates the
  // clause first, so `.*` is bounded to a single clause.
  const r = httpArmPredicates("WHERE correlation_id = 'abc' AND level IN ('error')")
  assert.equal(r.spans, " WHERE correlation_id = 'abc' AND (status_code >= 500)")
  assert.equal(r.logs, " AND (correlation_id = 'abc' AND level IN ('error'))")
  assert.deepEqual(r.unsupported, [])
})

test('bare level equality is not a compiler shape and fails loudly', () => {
  // compileFilterToQuery emits `level IN (...)` only; a bare `level = 'x'` is
  // an unknown shape that must land in `unsupported` (skips the HTTP queries)
  // rather than being silently ignored on one arm.
  const r = httpArmPredicates("WHERE correlation_id = 'abc' AND level = 'error'")
  assert.equal(r.spans, " WHERE correlation_id = 'abc'")
  assert.equal(r.logs, " AND (correlation_id = 'abc')")
  assert.deepEqual(r.unsupported, ["level = 'error'"])
})

test('correlation_id with escaped quotes and AND in the value stays one literal', () => {
  const r = httpArmPredicates("WHERE correlation_id = 'o''ne AND two' AND message LIKE '%api%'")
  assert.equal(r.spans, " WHERE correlation_id = 'o''ne AND two' AND (name LIKE '%api%' OR route LIKE '%api%')")
  assert.equal(r.logs, " AND (correlation_id = 'o''ne AND two' AND message LIKE '%api%')")
  assert.deepEqual(r.unsupported, [])
})

test('mixed realistic filter translates every clause on both arms', () => {
  const r = httpArmPredicates(
    "WHERE service IN ('web','api') AND timestamp > 1700000000000000 AND level IN ('error','warn') AND line >= 400",
  )
  assert.equal(
    r.spans,
    " WHERE service IN ('web','api') AND start_time > 1700000000000000 AND (status_code >= 500 OR status_code >= 400 AND status_code < 500) AND status_code >= 400",
  )
  assert.equal(
    r.logs,
    " AND (service IN ('web','api') AND timestamp > 1700000000000000 AND level IN ('error','warn') AND CAST(json_get_str(attributes, 'http.status_code') AS INT) >= 400)",
  )
  assert.deepEqual(r.unsupported, [])
})

test('unrecognized shapes are reported as unsupported, never silently dropped', () => {
  const r = httpArmPredicates("WHERE route = '/checkout' AND service IN ('web')")
  assert.equal(r.spans, ' WHERE service IN (\'web\')')
  assert.equal(r.logs, " AND (service IN ('web'))")
  assert.deepEqual(r.unsupported, ["route = '/checkout'"])
})

test('empty input produces empty arms and no unsupported clauses', () => {
  const r = httpArmPredicates('')
  assert.equal(r.spans, '')
  assert.equal(r.logs, '')
  assert.deepEqual(r.unsupported, [])
})

test('level values the HTTP middleware never emits match nothing on both arms', () => {
  const r = httpArmPredicates("WHERE level IN ('debug')")
  assert.equal(r.spans, ' WHERE status_code < 0')
  assert.equal(r.logs, " AND (level IN ('debug'))")
  assert.deepEqual(r.unsupported, [])
})

test('quoted line status chip maps to status_code on both arms', () => {
  const r = httpArmPredicates("WHERE line = '500'")
  assert.equal(r.spans, ' WHERE status_code = 500')
  assert.equal(r.logs, " AND (CAST(json_get_str(attributes, 'http.status_code') AS INT) = 500)")
  assert.deepEqual(r.unsupported, [])
})
