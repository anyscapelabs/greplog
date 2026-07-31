import { describe, it, expect, beforeEach } from 'vitest';
import { greplog, resetState } from '../src/index';
import defaultGreplog from '../src/index';

describe('manual API', () => {
  beforeEach(() => {
    resetState();
  });

  it('exports all four level functions on named greplog object', () => {
    expect(typeof greplog.error).toBe('function');
    expect(typeof greplog.warn).toBe('function');
    expect(typeof greplog.info).toBe('function');
    expect(typeof greplog.debug).toBe('function');
    expect(typeof greplog.init).toBe('function');
  });

  it('exports all four level functions on default greplog export', () => {
    expect(typeof defaultGreplog.error).toBe('function');
    expect(typeof defaultGreplog.warn).toBe('function');
    expect(typeof defaultGreplog.info).toBe('function');
    expect(typeof defaultGreplog.debug).toBe('function');
    expect(typeof defaultGreplog.init).toBe('function');
  });

  it('accepts details parameter optionally and fails open without init()', () => {
    expect(() => greplog.error('Payment failed', { orderId: '123' })).not.toThrow();
    expect(() => greplog.warn('Retrying request', { attempt: '2' })).not.toThrow();
    expect(() => greplog.info('no details')).not.toThrow();
    expect(() => greplog.debug('with details', { key: 'val' })).not.toThrow();
  });

  it('supports init with service option alias', () => {
    expect(() => greplog.init({ service: 'api-backend' })).not.toThrow();
    expect(() => greplog.info('Server started', { port: '4000' })).not.toThrow();
  });

  it('accepts various message types', () => {
    expect(() => greplog.error('')).not.toThrow();
    expect(() => greplog.warn('error occurred')).not.toThrow();
    expect(() => greplog.info('count is 42')).not.toThrow();
  });
});

