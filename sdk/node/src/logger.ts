export interface GreplogConfig {
  /** Source application name, e.g. `"payment-worker"`. Read from `GREPLOG_SERVICE_NAME` when omitted. */
  service?: string
  /** Deployment environment, e.g. `"production"`. Read from `GREPLOG_ENV` when omitted. */
  env?: string
  /** Base URL of the Greplog ingest server. Read from `GREPLOG_URL` when omitted. */
  endpoint?: string
  /** Number of buffered records that trigger an immediate flush. */
  batchSize?: number
  /** Interval (ms) between periodic flushes. */
  flushIntervalMs?: number
  /** Hard cap on buffered records; oldest records are dropped beyond this. */
  maxQueueSize?: number
}

/** Wire record sent to `POST /api/log`, matching the Rust `LogRecord` schema. */
export interface LogRecord {
  /** Microseconds since the UNIX epoch. */
  timestamp_us: number
  /** Correlation id grouping logs from one worker job or HTTP request. */
  trace_id: string | null
  /** Severity level, e.g. `"INFO"`, `"WARN"`, `"ERROR"`. */
  level: string
  /** Source application or worker. */
  service: string
  /** Human-readable summary of what happened. */
  message: string
  /** Stringified JSON payload (request body, stack trace, or worker job). */
  raw_body: string | null
}

const DEFAULT_ENDPOINT = process.env.GREPLOG_URL ?? 'http://127.0.0.1:5050'

/** Longest accepted service name: it becomes a storage partition directory. */
export const SERVICE_NAME_MAX_LEN = 64

/**
 * Mirrors the server's ingest rule exactly: the service name becomes a
 * `service=<name>` storage directory, so `/`, `\`, `..`, whitespace and
 * non-ASCII bytes are rejected before anything is queued. Checking here
 * turns a silent every-record-rejected loop into one loud startup error.
 */
export function isValidServiceName(service: string): boolean {
  return (
    service.length >= 1 &&
    service.length <= SERVICE_NAME_MAX_LEN &&
    /^[A-Za-z0-9._-]+$/.test(service)
  )
}

/**
 * Returns true for HTTP statuses worth retrying: 5xx (server-side failure)
 * and 429 (rate limited). Any other 4xx means the request itself was
 * rejected and resending the same bytes would just fail again.
 */
function isRetryableStatus(status: number): boolean {
  return status === 429 || status >= 500
}

function safeStringify(value: unknown): string | null {
  if (value == null) return null
  const seen = new WeakSet<object>()
  try {
    const json = JSON.stringify(value, (_key, current: unknown) => {
      if (typeof current === 'bigint') return current.toString()
      if (typeof current === 'object' && current !== null) {
        if (seen.has(current)) return '[Circular]'
        seen.add(current)
      }
      return current
    })
    return json ?? String(value)
  } catch {
    return String(value)
  }
}

export class GreplogClient {
  private queue: LogRecord[] = []
  private config: {
    service: string
    env: string
    endpoint: string
    batchSize: number
    flushIntervalMs: number
    maxQueueSize: number
  }
  private timer: NodeJS.Timeout | null = null
  private flushing: Promise<void> | null = null
  private dropped = 0

  constructor(config: GreplogConfig = {}) {
    const base = (config.endpoint ?? DEFAULT_ENDPOINT).replace(/\/+$/, '')
    // Read at construction, not import time, so late-set env vars still
    // apply. An empty env value counts as unset, matching the other SDKs.
    const service = config.service ?? (process.env.GREPLOG_SERVICE_NAME || 'node-app')
    if (!isValidServiceName(service)) {
      throw new Error(
        `greplog: invalid service name ${JSON.stringify(service)}: use 1 to ` +
          `${SERVICE_NAME_MAX_LEN} characters of a-z, A-Z, 0-9, '_', '.' or '-'`,
      )
    }
    this.config = {
      service,
      env: config.env ?? process.env.GREPLOG_ENV ?? 'development',
      endpoint: `${base}/api/log`,
      batchSize: config.batchSize ?? 100,
      flushIntervalMs: config.flushIntervalMs ?? 500,
      maxQueueSize: config.maxQueueSize ?? 10_000,
    }
    this.startTimer()
  }

  /** Accesses the service this client logs for. */
  getService(): string {
    return this.config.service
  }

  /** Number of records dropped because the queue exceeded `maxQueueSize`. */
  getDroppedCount(): number {
    return this.dropped
  }

  /** Pushes an event into the in-memory queue. Never throws. */
  public track(level: string, message: unknown, meta: Record<string, unknown> = {}): void {
    const { trace_id, ...payload } = meta
    const record: LogRecord = {
      timestamp_us: Date.now() * 1000,
      trace_id: typeof trace_id === 'string' ? trace_id : null,
      level,
      service: this.config.service,
      message: typeof message === 'string' ? message : safeStringify(message) ?? '',
      raw_body:
        Object.keys(payload).length > 0 ? safeStringify(payload) : null,
    }

    this.queue.push(record)

    if (this.queue.length > this.config.maxQueueSize) {
      this.queue.shift()
      this.dropped++
    }

    if (this.queue.length >= this.config.batchSize) {
      void this.flush()
    }
  }

  /** Sends the buffered queue to the ingest server. Fire-and-forget. */
  public flush(): Promise<void> {
    this.flushInternal()
    return this.flushing ?? Promise.resolve()
  }

  private flushInternal(): void {
    if (this.queue.length === 0 || this.flushing) return

    const batch = this.queue
    this.queue = []

    this.flushing = fetch(this.config.endpoint, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(batch),
    })
      .then((response) => {
        if (response.ok) return

        if (isRetryableStatus(response.status)) {
          // Transient server-side failure (5xx) or the server is asking us to
          // back off (429): the batch was never durably accepted, so it goes
          // back on the queue exactly like a network failure would.
          this.requeue(batch)
          return
        }

        // A 4xx means the server rejected this exact payload (bad JSON,
        // over the size limit, ...); retrying identical bytes will only fail
        // the same way forever, so the batch is dropped rather than looped.
        this.dropped += batch.length
      })
      .catch(() => {
        // Backend unreachable: re-queue silently, capping the buffer so the
        // caller's process memory never grows without bound.
        this.requeue(batch)
      })
      .finally(() => {
        this.flushing = null
      })
  }

  private requeue(batch: LogRecord[]): void {
    const merged = batch.concat(this.queue)
    if (merged.length > this.config.maxQueueSize) {
      this.dropped += merged.length - this.config.maxQueueSize
      merged.splice(0, merged.length - this.config.maxQueueSize)
    }
    this.queue = merged
  }

  private startTimer(): void {
    this.timer = setInterval(() => this.flushInternal(), this.config.flushIntervalMs)
    // Prevents the timer from keeping the Node process alive.
    this.timer.unref()
  }
}