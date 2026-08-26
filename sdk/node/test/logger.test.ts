import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { GreplogClient, isValidServiceName } from '../src/logger.js'

const ENDPOINT = 'http://127.0.0.1:5050/api/log'

function jsonBody(args: unknown[]): Array<Record<string, unknown>> {
  return JSON.parse(String(args[1].body)) as Array<Record<string, unknown>>
}

describe('GreplogClient', () => {
  beforeEach(() => {
    vi.unstubAllGlobals()
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.useRealTimers()
  })

  it('posts records to <endpoint>/api/log with the wire schema', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)

    const client = new GreplogClient({
      service: 'user-service',
      endpoint: 'http://127.0.0.1:5050',
      batchSize: 1,
    })
    client.track('ERROR', 'Payment failed', { error: 'card_declined' })
    await client.flush()

    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(fetchMock.mock.calls[0][0]).toBe(ENDPOINT)

    const [record] = jsonBody(fetchMock.mock.calls[0])
    expect(record.service).toBe('user-service')
    expect(record.level).toBe('ERROR')
    expect(record.message).toBe('Payment failed')
    expect(record.raw_body).toBe('{"error":"card_declined"}')
    expect(record.trace_id).toBeNull()
    expect(record.timestamp_us).toBeGreaterThan(0)
  })

  it('flushes only once the batch size is reached', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)

    const client = new GreplogClient({ service: 'svc', batchSize: 3 })
    client.track('INFO', 'one')
    client.track('INFO', 'two')
    expect(fetchMock).not.toHaveBeenCalled()

    client.track('INFO', 'three')
    await client.flush()

    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(jsonBody(fetchMock.mock.calls[0])).toHaveLength(3)
  })

  it('flushes on the timer interval when the buffer never fills', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)
    vi.useFakeTimers()

    const client = new GreplogClient({
      service: 'svc',
      batchSize: 100,
      flushIntervalMs: 500,
    })
    client.track('INFO', 'lone record')
    expect(fetchMock).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(500)

    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(jsonBody(fetchMock.mock.calls[0])).toHaveLength(1)
  })

  it('moves trace_id out of raw_body', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)

    const client = new GreplogClient({ service: 'svc', batchSize: 1 })
    client.track('INFO', 'job done', { trace_id: 'job_abc', extra: 1 })
    await client.flush()

    const [record] = jsonBody(fetchMock.mock.calls[0])
    expect(record.trace_id).toBe('job_abc')
    expect(record.raw_body).toBe('{"extra":1}')
  })

  it('drops the oldest records beyond maxQueueSize', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)

    const client = new GreplogClient({
      service: 'svc',
      batchSize: 100,
      maxQueueSize: 3,
    })
    for (const n of ['1', '2', '3', '4', '5']) client.track('INFO', n)

    expect(client.getDroppedCount()).toBe(2)

    await client.flush()
    const messages = jsonBody(fetchMock.mock.calls[0]).map(
      (row) => row.message,
    )
    expect(messages).toEqual(['3', '4', '5'])
  })

  it('re-queues and retries silently when the backend is unreachable', async () => {
    const fetchMock = vi
      .fn()
      .mockRejectedValueOnce(new Error('ECONNREFUSED'))
      .mockResolvedValueOnce(new Response(null, { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)

    const client = new GreplogClient({ service: 'svc', batchSize: 1 })
    client.track('INFO', 'boom')
    await client.flush()
    await client.flush()

    expect(fetchMock).toHaveBeenCalledTimes(2)
    expect(jsonBody(fetchMock.mock.calls[1])).toHaveLength(1)
    expect(client.getDroppedCount()).toBe(0)
  })

  it('re-queues and retries when the server returns a 500', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 500 }))
      .mockResolvedValueOnce(new Response(null, { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)

    const client = new GreplogClient({ service: 'svc', batchSize: 1 })
    client.track('ERROR', 'wal write failed upstream')
    await client.flush()
    await client.flush()

    expect(fetchMock).toHaveBeenCalledTimes(2)
    expect(jsonBody(fetchMock.mock.calls[1])).toHaveLength(1)
    expect(client.getDroppedCount()).toBe(0)
  })

  it('re-queues and retries when the server returns a 429', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 429 }))
      .mockResolvedValueOnce(new Response(null, { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)

    const client = new GreplogClient({ service: 'svc', batchSize: 1 })
    client.track('INFO', 'rate limited')
    await client.flush()
    await client.flush()

    expect(fetchMock).toHaveBeenCalledTimes(2)
    expect(client.getDroppedCount()).toBe(0)
  })

  it('drops the batch instead of retrying forever on a 4xx rejection', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 422 }))
    vi.stubGlobal('fetch', fetchMock)

    const client = new GreplogClient({ service: 'svc', batchSize: 1 })
    client.track('INFO', 'malformed somehow')
    await client.flush()

    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(client.getDroppedCount()).toBe(1)

    // The rejected batch must not still be sitting in the queue waiting to
    // be resent on the next flush.
    await client.flush()
    expect(fetchMock).toHaveBeenCalledTimes(1)
  })

  it('stringifies non-string messages and never rejects', async () => {
    const fetchMock = vi.fn().mockRejectedValue(new Error('down'))
    vi.stubGlobal('fetch', fetchMock)

    const client = new GreplogClient({ service: 'svc', batchSize: 1 })
    expect(() => client.track('INFO', { deep: { nested: 1 } })).not.toThrow()
    await client.flush()
  })
})
describe('service name validation', () => {
  afterEach(() => {
    vi.unstubAllEnvs()
  })

  it('accepts the names the server accepts', () => {
    expect(isValidServiceName('auth-api')).toBe(true)
    expect(isValidServiceName('payment_worker.2')).toBe(true)
    expect(isValidServiceName('A-1_2.b')).toBe(true)
    expect(isValidServiceName('x'.repeat(64))).toBe(true)
  })

  it('rejects path traversal and oversized names', () => {
    for (const name of ['', '../evil', '..\\windows', 'a/b', 'has space', 'süß', 'x'.repeat(65)]) {
      expect(isValidServiceName(name)).toBe(false)
    }
  })

  it('throws at construction with a message naming the rule', () => {
    expect(() => new GreplogClient({ service: '../evil' })).toThrowError(
      /invalid service name "\.\.\/evil".*1 to 64 characters/,
    )
  })

  it('still validates when the service comes from the environment or the default', () => {
    vi.stubEnv('GREPLOG_SERVICE_NAME', '../from-env')
    expect(() => new GreplogClient({})).toThrowError(/invalid service name/)
    vi.stubEnv('GREPLOG_SERVICE_NAME', '')
    const fallback = new GreplogClient({})
    expect(fallback.getService()).toBe('node-app')
  })
})
