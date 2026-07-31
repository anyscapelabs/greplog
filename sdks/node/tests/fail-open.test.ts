import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import * as http from 'http';
import { greplog } from '../src/index';

describe('fail-open — no agent running', () => {
  beforeEach(() => {
    greplog.resetState();
  });

  afterEach(() => {
    greplog.resetState();
  });

  it('init() does not throw when no agent is running', () => {
    expect(() => greplog.init()).not.toThrow();
  });

  it('error() does not throw before init()', () => {
    expect(() => greplog.error('test error')).not.toThrow();
  });

  it('warn() does not throw before init()', () => {
    expect(() => greplog.warn('test warn')).not.toThrow();
  });

  it('info() does not throw before init()', () => {
    expect(() => greplog.info('test info')).not.toThrow();
  });

  it('debug() does not throw before init()', () => {
    expect(() => greplog.debug('test debug')).not.toThrow();
  });

  it('manual API with details does not throw', () => {
    expect(() => greplog.error('err', { key: 'val' })).not.toThrow();
  });

  it('init() followed by manual API does not throw', () => {
    greplog.init({ serviceName: 'test-svc' });
    expect(() => greplog.info('after init')).not.toThrow();
    expect(() => greplog.error('after init error')).not.toThrow();
  });

  it('init() called twice is safe (no double-registration)', () => {
    expect(() => {
      greplog.init({ serviceName: 'test-svc' });
      greplog.init({ serviceName: 'test-svc' });
    }).not.toThrow();
  });

  it('error() with details containing sensitive fields does not throw', () => {
    expect(() => greplog.error('login failed', { password: 'hunter2' })).not.toThrow();
  });

  it('all public functions return undefined', () => {
    expect(greplog.error('test')).toBeUndefined();
    expect(greplog.warn('test')).toBeUndefined();
    expect(greplog.info('test')).toBeUndefined();
    expect(greplog.debug('test')).toBeUndefined();
    expect(greplog.init({ serviceName: 's' })).toBeUndefined();
  });

  it('HTTP request through patched server behaves identically with no agent (no hang, no extra latency)', async () => {
    greplog.init({ serviceName: 'fail-open-http', socketPath: '/non/existent/path/greplog.sock', tcpPort: 59999 });

    const server = http.createServer((_req, res) => {
      res.writeHead(200, { 'Content-Type': 'text/plain' });
      res.end('ok');
    });

    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
    const port = (server.address() as any).port;

    const start = Date.now();
    const res = await new Promise<{ status: number; body: string }>((resolve, reject) => {
      http.get({ hostname: '127.0.0.1', port, path: '/test' }, (response) => {
        let body = '';
        response.on('data', (chunk) => body += chunk);
        response.on('end', () => resolve({ status: response.statusCode ?? 0, body }));
      }).on('error', reject);
    });

    const duration = Date.now() - start;

    expect(res.status).toBe(200);
    expect(res.body).toBe('ok');
    expect(duration).toBeLessThan(2000); // Fast, no synchronous connection blocking

    await new Promise<void>((resolve) => server.close(() => resolve()));
  });
});
