import { state, detectServiceName, generateInstanceId, pushEvent } from './internal';
import { Transport } from './transport';
import { redactAttributes } from './redact';
import { initProtobuf } from './serialize';
import { patchHttp, patchConsole, registerErrorHooks, resetPatcherFlags } from './patchers';
import { detectFramework, writeConfig } from './detect';
import { generateULID } from './ulid';

export interface GreplogOptions {
  captureBodies?: boolean;
  captureConsoleLevel?: 'debug' | 'info' | 'warn' | 'error';
  serviceName?: string;
  socketPath?: string;
  tcpPort?: number;
}

function getTimestampNs(): number {
  return Date.now() * 1_000_000;
}

function buildEvent(
  message: string,
  level: string,
  details?: Record<string, string>,
): void {
  try {
    const serviceName = state.config?.service_name ?? detectServiceName();
    const attrs = details ? redactAttributes({ ...details }) : {};

    pushEvent({
      service_name: serviceName,
      message,
      level,
      timestamp_ns: getTimestampNs(),
      logger_name: 'greplog',
      event_id: generateULID(),
      attributes: attrs,
    });
  } catch {
    // fail-open: never throw
  }
}

export function error(message: string, details?: Record<string, string>): void {
  buildEvent(message, 'error', details);
}

export function warn(message: string, details?: Record<string, string>): void {
  buildEvent(message, 'warn', details);
}

export function info(message: string, details?: Record<string, string>): void {
  buildEvent(message, 'info', details);
}

export function debug(message: string, details?: Record<string, string>): void {
  buildEvent(message, 'debug', details);
}

export function init(options?: GreplogOptions): void {
  try {
    if (state.initialized) return;
    state.initialized = true;

    initProtobuf();

    const serviceName = options?.serviceName ?? detectServiceName();
    const instanceId = generateInstanceId();

    state.config = {
      service_name: serviceName,
      instance_id: instanceId,
      captureBodies: options?.captureBodies ?? false,
      captureConsoleLevel: options?.captureConsoleLevel ?? 'warn',
    };

    const transport = new Transport({
      socketPath: options?.socketPath,
      tcpPort: options?.tcpPort,
    });
    state.transport = transport;

    registerErrorHooks();
    patchHttp();
    patchConsole();

    // Framework detection (best-effort, must never throw)
    try {
      const detection = detectFramework();
      writeConfig(detection);
    } catch {
      // fail-open
    }

    transport.connect();
  } catch {
    // fail-open: init must never throw
    state.initialized = false;
  }
}

export function shutdown(): void {
  try {
    if (state.transport) {
      state.transport.destroy();
      state.transport = null;
    }
    state.initialized = false;
  } catch {
    // fail-open
  }
}

export function resetState(): void {
  shutdown();
  state.config = null;
  state.eventQueue = [];
  state.batchSeq = 0;
  state.warned = false;
  resetPatcherFlags();
}
