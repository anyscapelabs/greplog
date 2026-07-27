import { describe, it, expect } from 'vitest';
import { redactAttributes, RedactionMode } from '../src/redact';

describe('redactAttributes', () => {
  it('redacts password with Full mode', () => {
    const result = redactAttributes({ password: 'my-secret-pass' });
    expect(result.password).toBe('[REDACTED]');
  });

  it('redacts token with Full mode', () => {
    const result = redactAttributes({ token: 'abc123' });
    expect(result.token).toBe('[REDACTED]');
  });

  it('redacts secret with Full mode', () => {
    const result = redactAttributes({ secret: 'my-secret' });
    expect(result.secret).toBe('[REDACTED]');
  });

  it('redacts email with Partial mode', () => {
    const result = redactAttributes({ email: 'user@example.com' });
    expect(result.email).toBe('us***om');
  });

  it('handles short strings in Partial mode', () => {
    const result = redactAttributes({ email: 'ab@c' });
    expect(result.email).toBe('[***]');
  });

  it('passes through non-sensitive keys unchanged', () => {
    const result = redactAttributes({ message: 'hello', count: '42' });
    expect(result.message).toBe('hello');
    expect(result.count).toBe('42');
  });

  it('handles empty attributes', () => {
    const result = redactAttributes({});
    expect(result).toEqual({});
  });

  it('supports custom redaction keys', () => {
    const result = redactAttributes(
      { api_key: 'supersecret' },
      { api_key: RedactionMode.Full },
    );
    expect(result.api_key).toBe('[REDACTED]');
  });

  it('is case-insensitive for key matching', () => {
    const result = redactAttributes({ Password: 'secret123', TOKEN: 'xyz' });
    expect(result.Password).toBe('[REDACTED]');
    expect(result.TOKEN).toBe('[REDACTED]');
  });

  it('returns deterministic Hash values', () => {
    const r1 = redactAttributes({ email: 'test@test.com' }, { email: RedactionMode.Hash });
    const r2 = redactAttributes({ email: 'test@test.com' }, { email: RedactionMode.Hash });
    expect(r1.email).toBe(r2.email);
  });
});
