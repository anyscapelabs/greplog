import { GreplogClient, type GreplogConfig } from './logger.js'
import { instrumentNode } from './instrument.js'

let clientInstance: GreplogClient | null = null
let instrumented = false

export type GreplogMeta = Record<string, unknown>

const log =
  (level: string) =>
  (message: unknown, meta: GreplogMeta = {}): void => {
    clientInstance?.track(level, message, meta)
  }

const greplog = {
  /**
   * Boots the client and auto-instruments the process.
   *
   * Accepts a `GreplogConfig` object, or the shorthand form
   * `init('payment-service', 'production')`. Missing values fall back to the
   * `GREPLOG_SERVICE_NAME`, `GREPLOG_ENV` and `GREPLOG_URL` environment
   * variables. Calling `init` more than once is a no-op.
   */
  init(config?: GreplogConfig | string, env?: string): void {
    if (clientInstance) return // Prevent double initialization
    const normalized: GreplogConfig =
      typeof config === 'string' ? { service: config, env } : (config ?? {})
    clientInstance = new GreplogClient(normalized)
    if (!instrumented) {
      instrumentNode(clientInstance)
      instrumented = true
    }
  },

  // Explicit methods for developers who want structured data
  trace: log('TRACE'),
  debug: log('DEBUG'),
  info: log('INFO'),
  warn: log('WARN'),
  error: log('ERROR'),
  fatal: log('FATAL'),

  /** Flushes all buffered records. Call before a graceful shutdown. */
  flush(): Promise<void> {
    return clientInstance?.flush() ?? Promise.resolve()
  },
}

export default greplog
export { GreplogClient }
export type { GreplogConfig, LogRecord } from './logger.js'