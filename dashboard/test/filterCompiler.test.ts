import { test } from 'node:test'
import assert from 'node:assert/strict'
import { compileCheckedToQuery } from '../src/hooks/useFilterState.ts'

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
