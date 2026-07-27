import * as protobuf from 'protobufjs';
import * as path from 'path';
import { generateULID } from './ulid';

let ProtoRoot: protobuf.Root | null = null;
let LogEventType: protobuf.Type | null = null;
let IngestBatchType: protobuf.Type | null = null;
let IngestResponseType: protobuf.Type | null = null;

export interface LogEventData {
  service_name: string;
  message: string;
  level: string;
  timestamp_ns: number;
  logger_name?: string;
  file?: string;
  line?: number;
  correlation_id?: string;
  attributes?: Record<string, string>;
  stack_trace?: string[];
  exception_type?: string;
  exception_message?: string;
  event_id?: string;
}

export interface IngestBatchData {
  service_name: string;
  instance_id: string;
  batch_seq: number;
  logs: LogEventData[];
  spans: any[];
  metrics: any[];
}

export function initProtobuf(protoPath?: string): void {
  if (ProtoRoot) return;

  const resolvedPath = protoPath ?? path.join(__dirname, '..', 'proto', 'greplog', 'v1', 'events.proto');
  ProtoRoot = protobuf.loadSync(resolvedPath);
  LogEventType = ProtoRoot.lookupType('greplog.v1.LogEvent');
  IngestBatchType = ProtoRoot.lookupType('greplog.v1.IngestBatch');
  IngestResponseType = ProtoRoot.lookupType('greplog.v1.IngestResponse');
}

/**
 * Convert a LogEventData (snake_case) to the camelCase keys that
 * protobufjs create() requires. protobufjs silently drops snake_case
 * keys — every multi-word field (event_id, timestamp_ns, etc.) must
 * be camelCased or it won't appear on the wire.
 */
function logEventToCamelCase(data: LogEventData): Record<string, any> {
  const payload: Record<string, any> = {
    serviceName: data.service_name,
    message: data.message,
    level: data.level,
    timestampNs: data.timestamp_ns,
    eventId: data.event_id || generateULID(),
  };

  if (data.logger_name) payload.loggerName = data.logger_name;
  if (data.file) payload.file = data.file;
  if (data.line !== undefined) payload.line = data.line;
  if (data.correlation_id) payload.correlationId = data.correlation_id;
  if (data.attributes && Object.keys(data.attributes).length > 0) {
    payload.attributes = data.attributes;
  }
  if (data.stack_trace && data.stack_trace.length > 0) {
    payload.stackTrace = data.stack_trace;
  }
  if (data.exception_type) payload.exceptionType = data.exception_type;
  if (data.exception_message) payload.exceptionMessage = data.exception_message;

  return payload;
}

export function encodeLogEvent(data: LogEventData): Uint8Array {
  if (!LogEventType) initProtobuf();

  const payload = logEventToCamelCase(data);
  const message = LogEventType!.create(payload);
  return LogEventType!.encode(message).finish() as Uint8Array;
}

export function encodeIngestBatch(data: IngestBatchData): Uint8Array {
  if (!IngestBatchType) initProtobuf();

  const logs = data.logs.map((log) => logEventToCamelCase(log));

  const message = IngestBatchType!.create({
    serviceName: data.service_name,
    instanceId: data.instance_id,
    batchSeq: data.batch_seq,
    logs,
    spans: data.spans ?? [],
    metrics: data.metrics ?? [],
  });

  return IngestBatchType!.encode(message).finish() as Uint8Array;
}

export function decodeIngestResponse(buf: Uint8Array): { accepted: boolean; events_count: number; error: string } | null {
  if (!IngestResponseType) initProtobuf();
  try {
    const msg = IngestResponseType!.decode(buf);
    const obj = IngestResponseType!.toObject(msg, { defaults: true }) as any;
    return { accepted: !!obj.accepted, events_count: obj.eventsCount ?? 0, error: obj.error ?? '' };
  } catch {
    return null;
  }
}

export function encodeLengthPrefixedFrame(payload: Uint8Array): Buffer {
  const len = Buffer.alloc(4);
  len.writeUInt32LE(payload.length);
  return Buffer.concat([len, Buffer.from(payload)]);
}
