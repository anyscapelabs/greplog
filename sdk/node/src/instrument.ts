import type { GreplogClient } from './logger.js'

export function instrumentNode(client: GreplogClient): void {
  // 1. Monkey-patch Console
  const originalLog = console.log
  const originalError = console.error
  const originalWarn = console.warn
  const originalInfo = console.info
  const originalDebug = console.debug

  console.log = (...args: unknown[]) => {
    client.track('INFO', args.join(' '))
    originalLog.apply(console, args)
  }

  console.info = (...args: unknown[]) => {
    client.track('INFO', args.join(' '))
    originalInfo.apply(console, args)
  }

  console.warn = (...args: unknown[]) => {
    client.track('WARN', args.join(' '))
    originalWarn.apply(console, args)
  }

  console.error = (...args: unknown[]) => {
    // Attempt to extract stack traces and clean messages from Error objects
    const first = args[0]
    const meta = first instanceof Error ? { stack: first.stack } : {}
    const message =
      first instanceof Error ? first.message : args.join(' ')
    client.track('ERROR', message, meta)
    originalError.apply(console, args)
  }

  console.debug = (...args: unknown[]) => {
    client.track('DEBUG', args.join(' '))
    originalDebug.apply(console, args)
  }

  // 2. Catch Unhandled Exceptions
  process.on('uncaughtException', (err) => {
    client.track('CRITICAL', err.message, {
      stack: err.stack,
      type: 'uncaughtException',
    })
    originalError.call(console, 'Uncaught Exception:', err)
    // Give the batch a micro-tick to flush, then exit non-zero.
    void client.flush().finally(() => process.exit(1))
  })

  process.on('unhandledRejection', (reason: unknown) => {
    client.track(
      'CRITICAL',
      reason instanceof Error ? reason.message : String(reason),
      {
        stack: reason instanceof Error ? reason.stack : undefined,
        type: 'unhandledRejection',
      },
    )
    originalError.call(console, 'Unhandled Rejection:', reason)
  })
}