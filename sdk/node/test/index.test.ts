import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

async function loadGreplog() {
  vi.resetModules()
  const mod = await import('../src/index.js')
  return mod.default as typeof import('../src/index.js')['default']
}

const CONSOLE_METHODS = ['log', 'info', 'warn', 'error', 'debug'] as const

describe('greplog singleton', () => {
  beforeEach(() => {
    vi.unstubAllGlobals()
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    delete process.env.GREPLOG_SERVICE_NAME
    delete process.env.GREPLOG_ENV
    delete process.env.GREPLOG_URL
  })

  it('reads service, env and url from the environment', async () => {
    process.env.GREPLOG_SERVICE_NAME = 'env-service'
    process.env.GREPLOG_ENV = 'staging'
    process.env.GREPLOG_URL = 'http://127.0.0.1:7777'
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)

    const greplog = await loadGreplog()
    greplog.init()
    greplog.info('from env')

    await greplog.flush()
    const body = JSON.parse(String(fetchMock.mock.calls[0][1].body))
    expect(fetchMock.mock.calls[0][0]).toBe('http://127.0.0.1:7777/api/log')
    expect(body[0].service).toBe('env-service')
  })

  it('accepts the init(service, env) shorthand', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)

    const greplog = await loadGreplog()
    greplog.init('payment-service', 'production')
    greplog.warn('retry', { attempt: 2 })

    await greplog.flush()
    const body = JSON.parse(String(fetchMock.mock.calls[0][1].body))
    expect(body[0].service).toBe('payment-service')
    expect(body[0].level).toBe('WARN')
    expect(body[0].raw_body).toBe('{"attempt":2}')
  })

  it('is a no-op when init is called twice', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)

    const greplog = await loadGreplog()
    greplog.init({ service: 'first' })
    greplog.init({ service: 'second' })
    greplog.fatal('boom', { code: 1 })

    await greplog.flush()
    const body = JSON.parse(String(fetchMock.mock.calls[0][1].body))
    expect(body[0].service).toBe('first')
  })

  it('offers all documented log levels', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)

    const greplog = await loadGreplog()
    greplog.init({ service: 'svc', batchSize: 6 })
    greplog.trace('t')
    greplog.debug('d')
    greplog.info('i')
    greplog.warn('w')
    greplog.error('e')
    greplog.fatal('f')

    await greplog.flush()
    const body = JSON.parse(String(fetchMock.mock.calls[0][1].body))
    expect(body.map((row: { level: string }) => row.level)).toEqual([
      'TRACE',
      'DEBUG',
      'INFO',
      'WARN',
      'ERROR',
      'FATAL',
    ])
  })

  it('reset console patches between tests', () => {
    for (const method of CONSOLE_METHODS) {
      expect(typeof console[method]).toBe('function')
    }
  })
})