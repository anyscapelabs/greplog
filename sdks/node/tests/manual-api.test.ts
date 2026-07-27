import { describe, it, expect } from 'vitest';
import * as greplog from '../src/index';

describe('manual API', () => {
  it('exports all four level functions', () => {
    expect(typeof greplog.error).toBe('function');
    expect(typeof greplog.warn).toBe('function');
    expect(typeof greplog.info).toBe('function');
    expect(typeof greplog.debug).toBe('function');
  });

  it('exports init as a function', () => {
    expect(typeof greplog.init).toBe('function');
  });

  it('accepts details parameter optionally', () => {
    expect(() => greplog.info('no details')).not.toThrow();
    expect(() => greplog.info('with details', { key: 'val' })).not.toThrow();
    expect(() => greplog.info('with empty details', {})).not.toThrow();
  });

  it('accepts various message types', () => {
    expect(() => greplog.error('')).not.toThrow();
    expect(() => greplog.warn('error occurred')).not.toThrow();
    expect(() => greplog.info('count is 42')).not.toThrow();
  });
});
