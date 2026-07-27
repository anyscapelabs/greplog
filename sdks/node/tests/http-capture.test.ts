import { describe, it, expect } from 'vitest';
import * as http from 'http';
import * as greplog from '../src/index';

function request(server: http.Server, path: string): Promise<{ status: number; body: string }> {
  return new Promise((resolve, reject) => {
    const addr = server.address();
    if (!addr || typeof addr === 'string') { reject(new Error('bad address')); return; }
    const req = http.request({
      hostname: '127.0.0.1', port: addr.port, path, method: 'GET',
    }, (res) => {
      let body = '';
      res.on('data', (c) => body += c);
      res.on('end', () => resolve({ status: res.statusCode ?? 0, body }));
    });
    req.on('error', reject);
    req.end();
  });
}

function listen(server: http.Server): Promise<number> {
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      const a = server.address();
      resolve(typeof a === 'object' && a ? a.port : 0);
    });
  });
}

describe('HTTP server patching', () => {
  it('raw http.Server handles requests with greplog active', async () => {
    greplog.init({ serviceName: 'http-test' });
    greplog.shutdown();

    const server = http.createServer((_req, res) => {
      res.writeHead(200, { 'Content-Type': 'text/plain' });
      res.end('ok');
    });

    const port = await listen(server);
    expect(port).toBeGreaterThan(0);

    const { status, body } = await request(server, '/test');
    expect(status).toBe(200);
    expect(body).toBe('ok');

    await new Promise<void>((r) => server.close(() => r()));
  });

  it('handles 500 status codes with patched http', async () => {
    const server = http.createServer((_req, res) => {
      res.writeHead(500);
      res.end('error');
    });

    const port = await listen(server);
    expect(port).toBeGreaterThan(0);

    const { status } = await request(server, '/fail');
    expect(status).toBe(500);

    await new Promise<void>((r) => server.close(() => r()));
  });
});

describe('console patching', () => {
  it('console.warn still works after greplog init', () => {
    greplog.init({ serviceName: 'console-test', captureConsoleLevel: 'warn' });
    greplog.shutdown();
    expect(() => { console.warn('warn test'); }).not.toThrow();
  });

  it('console.error still works after greplog init', () => {
    expect(() => { console.error('error test'); }).not.toThrow();
  });

  it('console.log is not captured by default', () => {
    const calls: unknown[][] = [];
    const orig = console.log;
    console.log = (...args) => calls.push(args);
    try {
      console.log('should work');
      expect(calls.length).toBe(1);
      expect(calls[0][0]).toBe('should work');
    } finally {
      console.log = orig;
    }
  });
});
