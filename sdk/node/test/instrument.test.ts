import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { instrumentNode } from '../src/instrument.js'
import type { GreplogClient } from '../src/logger.js'

function makeClient() {
  return {
    track: vi.fn(),
    flush: vi.fn(),
  } as unknown as GreplogClient
}

const CONSOLE_METHODS = ['log', 'info', 'warn', 'error', 'debug'] as const

describe('instrumentNode', () => {
  const originals: Record<string, (...args: never[]) => void> = {}

  beforeEach(() => {
    for (const method of CONSOLE_METHODS) {
      originals[method] = console[method]
    }
  })

  afterEach(() => {
    for (const method of CONSOLE_METHODS) {
      console[method] = originals[method]
    }
    vi.restoreAllMocks()
  })

  it('captures console.log while preserving original output', () => {
    const client = makeClient()
    const output = vi.fn()
    console.log = output as unknown as typeof console.log

    instrumentNode(client)
    console.log('hello', 42)

    expect(client.track).toHaveBeenCalledWith('INFO', 'hello 42')
    expect(output).toHaveBeenCalledWith('hello', 42)
  })

  it.each([
    ['info', 'INFO'],
    ['warn', 'WARN'],
    ['debug', 'DEBUG'],
  ] as const)('captures console.%s as %s', (method, level) => {
    const client = makeClient()
    const output = vi.fn()
    console[method] = output as unknown as typeof console[typeof method]

    instrumentNode(client)
    console[method]('something happened')

    expect(client.track).toHaveBeenCalledWith(level, 'something happened')
    expect(output).toHaveBeenCalledWith('something happened')
  })

  it('captures console.error as ERROR with a meta object', () => {
    const client = makeClient()
    console.error = vi.fn() as unknown as typeof console.error

    instrumentNode(client)
    console.error('something happened')

    expect(client.track).toHaveBeenCalledWith(
      'ERROR',
      'something happened',
      {},
    )
  })

  it('extracts a stack trace when console.error receives an Error', () => {
    const client = makeClient()
    console.error = vi.fn() as unknown as typeof console.error

    instrumentNode(client)
    const err = new Error('oh no')
    console.error(err)

    expect(client.track).toHaveBeenCalledWith(
      'ERROR',
      'oh no',
      expect.objectContaining({ stack: err.stack }),
    )
  })

  it('captures unhandled rejections as CRITICAL', () => {
    const client = makeClient()
    instrumentNode(client)

    process.emit('unhandledRejection', new Error('async boom'))

    expect(client.track).toHaveBeenCalledWith(
      'CRITICAL',
      'async boom',
      expect.objectContaining({ type: 'unhandledRejection' }),
    )
  })
})