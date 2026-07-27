import * as http from 'http';
import * as https from 'https';
import { IncomingMessage, ServerResponse } from 'http';
import { state, pushEvent } from './internal';
import { redactAttributes, redactHeaders } from './redact';
import { generateULID } from './ulid';

let httpPatched = false;
let consolePatched = false;
let hooksRegistered = false;

export function resetPatcherFlags(): void {
  httpPatched = false;
  consolePatched = false;
  hooksRegistered = false;
}

function generateSpanId(): string {
  return generateULID();
}

function safeString(val: any): string {
  if (typeof val === 'string') return val;
  if (typeof val === 'number' || typeof val === 'boolean') return String(val);
  return '';
}

export function patchHttp(): void {
  if (httpPatched) return;
  httpPatched = true;

  try {
    const originalHttpEmit: Function = http.Server.prototype.emit;
    http.Server.prototype.emit = function (): boolean {
      const args = arguments;
      try {
        if (args.length >= 2 && (args[0] === 'request' || args[0] === String.fromCharCode(114, 101, 113, 117, 101, 115, 116))) {
          captureHttpRequest(args[1] as IncomingMessage, args[2] as ServerResponse);
        }
      } catch {
        // fail-open
      }
      return originalHttpEmit.apply(this, args as any);
    };

    const originalHttpsEmit: Function = https.Server.prototype.emit;
    https.Server.prototype.emit = function (): boolean {
      const args = arguments;
      try {
        if (args.length >= 2 && (args[0] === 'request' || args[0] === String.fromCharCode(114, 101, 113, 117, 101, 115, 116))) {
          captureHttpRequest(args[1] as IncomingMessage, args[2] as ServerResponse);
        }
      } catch {
        // fail-open
      }
      return originalHttpsEmit.apply(this, args as any);
    };
  } catch {
    // fail-open
  }
}

function captureHttpRequest(req: IncomingMessage, res: ServerResponse): void {
  try {
    const startTime = Date.now();
    const requestId = generateSpanId();

    const onFinish = () => {
      try {
        res.removeListener('finish', onFinish);
        const latencyMs = Date.now() - startTime;

        const serviceName = state.config?.service_name ?? 'unknown-service';
        const method = safeString(req.method);
        const url = safeString(req.url);
        const statusCode = res.statusCode ?? 0;

        const bodyCaptured = state.config?.captureBodies === true;

        const attrs: Record<string, string> = {
          'http.method': method,
          'http.route': url,
          'http.status_code': String(statusCode),
          'http.latency_ms': String(latencyMs),
        };

        if (bodyCaptured) {
          attrs['http.request.body'] = '(body captured)';
        }

        const headers: Record<string, string> = {};
        if (req.headers) {
          for (const [key, val] of Object.entries(req.headers)) {
            headers[key] = Array.isArray(val) ? val.join(', ') : (val ?? '');
          }
        }
        const redactedHeaders = redactHeaders(headers);

        pushEvent({
          service_name: serviceName,
          message: `${method} ${url} → ${statusCode} (${latencyMs}ms)`,
          level: statusCode >= 500 ? 'error' : statusCode >= 400 ? 'warn' : 'info',
          timestamp_ns: startTime * 1_000_000,
          logger_name: 'greplog.http',
          correlation_id: '',
          event_id: generateULID(),
          attributes: redactAttributes({ ...attrs, ...redactedHeaders }),
        });
      } catch {
        // fail-open
      }
    };

    res.on('finish', onFinish);
  } catch {
    // fail-open
  }
}

export function patchConsole(): void {
  if (consolePatched) return;
  consolePatched = true;

  const level = state.config?.captureConsoleLevel ?? 'warn';

  if (level === 'debug' || level === 'info' || level === 'warn' || level === 'error') {
    const originalError = console.error;
    console.error = function (...args: any[]) {
      try {
        const message = args.map((a: any) => safeString(a)).join(' ');
        pushEvent({
          service_name: state.config?.service_name ?? 'unknown-service',
          message,
          level: 'error',
          timestamp_ns: Date.now() * 1_000_000,
          logger_name: 'console',
          event_id: generateULID(),
          attributes: {},
        });
      } catch {
        // fail-open: never throw
      }
      originalError.apply(console, args);
    };
  }

  if (level === 'debug' || level === 'info' || level === 'warn') {
    const originalWarn = console.warn;
    console.warn = function (...args: any[]) {
      try {
        const message = args.map((a: any) => safeString(a)).join(' ');
        pushEvent({
          service_name: state.config?.service_name ?? 'unknown-service',
          message,
          level: 'warn',
          timestamp_ns: Date.now() * 1_000_000,
          logger_name: 'console',
          event_id: generateULID(),
          attributes: {},
        });
      } catch {
        // fail-open
      }
      originalWarn.apply(console, args);
    };
  }
}

export function registerErrorHooks(): void {
  if (hooksRegistered) return;
  hooksRegistered = true;

  process.on('uncaughtException', (err: Error) => {
    try {
      pushEvent({
        service_name: state.config?.service_name ?? 'unknown-service',
        message: err?.message ?? String(err),
        level: 'fatal',
        timestamp_ns: Date.now() * 1_000_000,
        logger_name: 'uncaughtException',
        stack_trace: err?.stack ? err.stack.split('\n') : [],
        exception_type: err?.name ?? 'Error',
        exception_message: err?.message ?? String(err),
        event_id: generateULID(),
        attributes: {},
      });
    } catch {
      // fail-open
    }
  });

  process.on('unhandledRejection', (reason: any) => {
    try {
      const err = reason instanceof Error ? reason : new Error(safeString(reason));
      pushEvent({
        service_name: state.config?.service_name ?? 'unknown-service',
        message: err.message,
        level: 'error',
        timestamp_ns: Date.now() * 1_000_000,
        logger_name: 'unhandledRejection',
        stack_trace: err.stack ? err.stack.split('\n') : [],
        exception_type: err.name,
        exception_message: err.message,
        event_id: generateULID(),
        attributes: {},
      });
    } catch {
      // fail-open
    }
  });
}
