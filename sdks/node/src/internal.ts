import { Transport } from './transport';
import { LogEventData } from './serialize';
import { redactAttributes } from './redact';
import { generateULID } from './ulid';
import * as fs from 'fs';

export interface GreplogConfig {
  service_name: string;
  instance_id: string;
  captureBodies: boolean;
  captureConsoleLevel: string;
}

export const state: {
  initialized: boolean;
  config: GreplogConfig | null;
  transport: Transport | null;
  eventQueue: LogEventData[];
  batchSeq: number;
  warned: boolean;
} = {
  initialized: false,
  config: null,
  transport: null,
  eventQueue: [],
  batchSeq: 0,
  warned: false,
};

export function detectServiceName(): string {
  try {
    const pkg = JSON.parse(fs.readFileSync('package.json', 'utf-8'));
    if (pkg.name) return pkg.name;
  } catch {
    // ignore
  }
  return 'unknown-service';
}

export function generateInstanceId(): string {
  return generateULID();
}

export function flushEventQueue(): void {
  try {
    if (!state.transport || state.eventQueue.length === 0) return;

    const events = state.eventQueue.splice(0, 100);
    const serviceName = state.config?.service_name ?? detectServiceName();
    const instanceId = state.config?.instance_id ?? generateInstanceId();

    for (const event of events) {
      event.attributes = redactAttributes(event.attributes ?? {});
      if (!event.event_id) {
        event.event_id = generateULID();
      }
    }

    state.batchSeq++;

    const { encodeIngestBatch } = require('./serialize');
    const batch = encodeIngestBatch({
      service_name: serviceName,
      instance_id: instanceId,
      batch_seq: state.batchSeq,
      logs: events,
      spans: [],
      metrics: [],
    });

    state.transport.send(batch);
  } catch {
    // fail-open
  }
}

export function pushEvent(event: LogEventData): void {
  try {
    if (!event.event_id) {
      event.event_id = generateULID();
    }
    state.eventQueue.push(event);
    if (state.eventQueue.length > 1000) {
      state.eventQueue.shift();
    }
    flushEventQueue();
  } catch {
    // fail-open
  }
}
