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
const DEFAULT_SERVICE = process.env.GREPLOG_SERVICE_NAME ?? 'node-app'

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
    this.config = {
      service: config.service ?? DEFAULT_SERVICE,
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
      .then(() => undefined)
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