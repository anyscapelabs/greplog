import { describe, it, expect, beforeAll, afterAll, beforeEach } from 'vitest';
import * as net from 'net';
import * as http from 'http';
import * as path from 'path';
import * as protobuf from 'protobufjs';
import { spawn } from 'child_process';
import * as fs from 'fs';

const protoPath = path.join(__dirname, '..', 'proto', 'greplog', 'v1', 'events.proto');
const ProtoRoot = protobuf.loadSync(protoPath);
const IngestBatchType = ProtoRoot.lookupType('greplog.v1.IngestBatch');

let mockAgent: net.Server;
let receivedBatches: any[] = [];
let agentPort = 0;

beforeAll(async () => {
  mockAgent = net.createServer((sock) => {
    let data = Buffer.alloc(0);
    sock.on('data', (chunk) => {
      data = Buffer.concat([data, chunk]);
      let offset = 0;
      while (offset + 4 <= data.length) {
        const len = data.readUInt32LE(offset);
        if (offset + 4 + len > data.length) break;
        const msg = IngestBatchType.decode(data.slice(offset + 4, offset + 4 + len));
        receivedBatches.push(IngestBatchType.toObject(msg, { defaults: true }));
        offset += 4 + len;
      }
      data = data.slice(offset);
    });
  });

  await new Promise<void>((resolve) => {
    mockAgent.listen(0, '127.0.0.1', () => {
      const addr = mockAgent.address() as net.AddressInfo;
      agentPort = addr.port;
      resolve();
    });
  });
});

afterAll(async () => {
  await new Promise<void>((resolve) => mockAgent.close(() => resolve()));
});

beforeEach(() => {
  receivedBatches = [];
});

function runNodeScript(scriptContent: string, fileName: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const scriptPath = path.join(__dirname, fileName);
    fs.writeFileSync(scriptPath, scriptContent);

    const proc = spawn('node', [scriptPath], { stdio: 'inherit' });
    proc.on('close', () => {
      if (fs.existsSync(scriptPath)) {
        try { fs.unlinkSync(scriptPath); } catch {}
      }
      setTimeout(resolve, 150);
    });
    proc.on('error', (err) => {
      if (fs.existsSync(scriptPath)) {
        try { fs.unlinkSync(scriptPath); } catch {}
      }
      reject(err);
    });
  });
}

describe('E2E Capture Tests', () => {
  it('captures express, fastify, and raw http requests without framework-specific setup', async () => {
    const script = `
      const { greplog } = require('../dist/index.js');
      const http = require('http');
      const express = require('express');
      const fastify = require('fastify')();

      greplog.init({ tcpPort: ${agentPort}, serviceName: 'http-app', socketPath: '/non/existent.sock' });

      const app = express();
      app.get('/express', (req, res) => res.send('ok'));
      const expressServer = app.listen(0, '127.0.0.1', async () => {
        const ePort = expressServer.address().port;
        
        fastify.get('/fastify', (request, reply) => { reply.send('ok'); });
        await fastify.listen({ port: 0, host: '127.0.0.1' });
        const fPort = fastify.server.address().port;

        const raw = http.createServer((req, res) => { res.end('ok'); });
        raw.listen(0, '127.0.0.1', async () => {
          const rPort = raw.address().port;

          const req = (port, path) => new Promise(r => {
            http.get({ hostname: '127.0.0.1', port, path }, (res) => {
              res.on('data', () => {});
              res.on('end', () => r());
            });
          });

          await req(ePort, '/express');
          await req(fPort, '/fastify');
          await req(rPort, '/raw');

          setTimeout(() => { process.exit(0); }, 300);
        });
      });
    `;

    await runNodeScript(script, 'test-http-all.js');

    const logs = receivedBatches.flatMap(b => b.logs);
    expect(logs.length).toBeGreaterThanOrEqual(3);

    const expressLog = logs.find(l => l.attributes?.['http.route'] === '/express');
    const fastifyLog = logs.find(l => l.attributes?.['http.route'] === '/fastify');
    const rawLog = logs.find(l => l.attributes?.['http.route'] === '/raw');

    expect(expressLog).toBeDefined();
    expect(fastifyLog).toBeDefined();
    expect(rawLog).toBeDefined();

    expect(expressLog?.attributes['http.method']).toBe('GET');
    expect(expressLog?.attributes['http.status_code']).toBe('200');
    expect(expressLog?.attributes['http.latency_ms']).toBeDefined();

    expect(expressLog?.eventId).toBeDefined();
    expect(expressLog?.eventId.length).toBeGreaterThan(0);
  });

  it('captures uncaught exceptions with stack trace and message', async () => {
    const script = `
      const { greplog } = require('../dist/index.js');
      greplog.init({ tcpPort: ${agentPort}, serviceName: 'uncaught-test', socketPath: '/non/existent.sock' });
      
      setTimeout(() => {
        try {
          throw new Error('Uncaught error test');
        } catch (err) {
          process.emit('uncaughtException', err);
        }
        setTimeout(() => { process.exit(0); }, 200);
      }, 100);
    `;

    await runNodeScript(script, 'test-uncaught.js');

    const logs = receivedBatches.flatMap(b => b.logs);
    const log = logs.find(l => l.message === 'Uncaught error test');
    expect(log).toBeDefined();
    expect(log?.level).toBe('fatal');
    expect(log?.exceptionType).toBe('Error');
    expect(log?.stackTrace).toBeDefined();
    expect(log?.stackTrace.length).toBeGreaterThan(0);
    expect(log?.eventId).toBeDefined();
  });

  it('captures unhandled rejections', async () => {
    const script = `
      const { greplog } = require('../dist/index.js');
      greplog.init({ tcpPort: ${agentPort}, serviceName: 'unhandled-test', socketPath: '/non/existent.sock' });
      
      setTimeout(() => {
        try {
          Promise.reject(new Error('Unhandled rejection test'));
        } catch {}
        process.emit('unhandledRejection', new Error('Unhandled rejection test'), Promise.resolve());
        setTimeout(() => { process.exit(0); }, 200);
      }, 100);
    `;

    await runNodeScript(script, 'test-unhandled.js');

    const logs = receivedBatches.flatMap(b => b.logs);
    const log = logs.find(l => l.message === 'Unhandled rejection test');
    expect(log).toBeDefined();
    expect(log?.level).toBe('error');
    expect(log?.exceptionType).toBe('Error');
  });

  it('does not capture request/response body by default', async () => {
    const script = `
      const { greplog } = require('../dist/index.js');
      const http = require('http');

      greplog.init({ tcpPort: ${agentPort}, serviceName: 'body-test', socketPath: '/non/existent.sock' });

      const raw = http.createServer((req, res) => {
        let body = '';
        req.on('data', c => body += c);
        req.on('end', () => res.end('ok'));
      });
      raw.listen(0, '127.0.0.1', () => {
        const port = raw.address().port;
        const req = http.request({ hostname: '127.0.0.1', port, path: '/', method: 'POST' }, (res) => {
          res.on('data', () => {});
          res.on('end', () => {
            setTimeout(() => { process.exit(0); }, 300);
          });
        });
        req.write('sensitive body content');
        req.end();
      });
    `;

    await runNodeScript(script, 'test-body.js');

    const logs = receivedBatches.flatMap(b => b.logs);
    const log = logs.find(l => l.attributes?.['http.method'] === 'POST');
    expect(log).toBeDefined();
    expect(log?.attributes['http.request.body']).toBeUndefined();
  });

  it('console patching respects default level (warn/error forwarded, log not forwarded)', async () => {
    const script = `
      const { greplog } = require('../dist/index.js');
      greplog.init({ tcpPort: ${agentPort}, serviceName: 'console-test', socketPath: '/non/existent.sock' });

      console.error('console error msg');
      console.warn('console warn msg');
      console.log('console log msg');

      setTimeout(() => { process.exit(0); }, 300);
    `;

    await runNodeScript(script, 'test-console.js');

    const logs = receivedBatches.flatMap(b => b.logs);
    const errorLog = logs.find(l => l.message === 'console error msg');
    const warnLog = logs.find(l => l.message === 'console warn msg');
    const logLog = logs.find(l => l.message === 'console log msg');

    expect(errorLog).toBeDefined();
    expect(warnLog).toBeDefined();
    expect(logLog).toBeUndefined(); // console.log is NOT forwarded by default
  });

  it('manual API works before and after init()', async () => {
    const script = `
      const { greplog } = require('../dist/index.js');

      // Call before init()
      greplog.error('manual before init');

      // Now call init()
      greplog.init({ tcpPort: ${agentPort}, serviceName: 'manual-test', socketPath: '/non/existent.sock' });

      // Call after init()
      greplog.info('manual after init');

      setTimeout(() => { process.exit(0); }, 300);
    `;

    await runNodeScript(script, 'test-manual-order.js');

    const logs = receivedBatches.flatMap(b => b.logs);
    const beforeLog = logs.find(l => l.message === 'manual before init');
    const afterLog = logs.find(l => l.message === 'manual after init');

    expect(beforeLog).toBeDefined();
    expect(afterLog).toBeDefined();
    expect(beforeLog?.eventId).toBeDefined();
    expect(afterLog?.eventId).toBeDefined();
  });

  it('redaction applies uniformly to manual and auto-captured events', async () => {
    const script = `
      const { greplog } = require('../dist/index.js');
      const http = require('http');

      greplog.init({ tcpPort: ${agentPort}, serviceName: 'redact-test', socketPath: '/non/existent.sock' });

      // Manual API with sensitive key
      greplog.error('user login', { password: 'raw-password-123' });

      // HTTP auto-capture with sensitive header
      const server = http.createServer((req, res) => res.end('ok'));
      server.listen(0, '127.0.0.1', () => {
        const port = server.address().port;
        const req = http.request({
          hostname: '127.0.0.1', port, path: '/auth', method: 'GET',
          headers: { password: 'http-header-secret' }
        }, (res) => {
          res.on('data', () => {});
          res.on('end', () => {
            setTimeout(() => { process.exit(0); }, 300);
          });
        });
        req.end();
      });
    `;

    await runNodeScript(script, 'test-redact-both.js');

    const logs = receivedBatches.flatMap(b => b.logs);
    const manualLog = logs.find(l => l.message === 'user login');
    const httpLog = logs.find(l => l.attributes?.['http.route'] === '/auth');

    expect(manualLog).toBeDefined();
    expect(manualLog?.attributes?.password).toBe('[REDACTED]');

    expect(httpLog).toBeDefined();
    expect(httpLog?.attributes?.password).toBe('[REDACTED]');
  });

  it('init() is idempotent and does not double-register handlers', async () => {
    const script = `
      const { greplog } = require('../dist/index.js');

      greplog.init({ tcpPort: ${agentPort}, serviceName: 'idempotent-test', socketPath: '/non/existent.sock' });
      greplog.init({ tcpPort: ${agentPort}, serviceName: 'idempotent-test', socketPath: '/non/existent.sock' });

      process.emit('uncaughtException', new Error('Single uncaught exception'));
      setTimeout(() => { process.exit(0); }, 300);
    `;

    await runNodeScript(script, 'test-idempotent.js');

    const logs = receivedBatches.flatMap(b => b.logs);
    const uncaughtLogs = logs.filter(l => l.message === 'Single uncaught exception');
    expect(uncaughtLogs.length).toBe(1);
  });
});
