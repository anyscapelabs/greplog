export enum RedactionMode {
  Full = 'Full',
  Partial = 'Partial',
  Hash = 'Hash',
}

const DEFAULT_REDACTED_KEYS: Record<string, RedactionMode> = {
  password: RedactionMode.Full,
  token: RedactionMode.Full,
  secret: RedactionMode.Full,
  email: RedactionMode.Partial,
};

function redactString(val: string, mode: RedactionMode): string {
  if (val.length === 0) return '';

  switch (mode) {
    case RedactionMode.Full:
      return '[REDACTED]';
    case RedactionMode.Partial: {
      if (val.length <= 4) return '[***]';
      const first = val.slice(0, 2);
      const last = val.slice(-2);
      return `${first}***${last}`;
    }
    case RedactionMode.Hash: {
      let hash = 0;
      for (let i = 0; i < val.length; i++) {
        const chr = val.charCodeAt(i);
        hash = ((hash << 5) - hash) + chr;
        hash |= 0;
      }
      return `[HASH:${(hash >>> 0).toString(16).padStart(8, '0')}]`;
    }
  }
}

function keyMatchesPattern(key: string, pattern: string): boolean {
  return key.toLowerCase().includes(pattern.toLowerCase());
}

export function redactAttributes(
  attrs: Record<string, string>,
  customKeys?: Record<string, RedactionMode>,
): Record<string, string> {
  const mergedKeys = { ...DEFAULT_REDACTED_KEYS, ...customKeys };
  const result: Record<string, string> = {};
  for (const [key, val] of Object.entries(attrs)) {
    let matched = false;
    for (const [pattern, mode] of Object.entries(mergedKeys)) {
      if (keyMatchesPattern(key, pattern)) {
        result[key] = redactString(val, mode);
        matched = true;
        break;
      }
    }
    if (!matched) {
      result[key] = val;
    }
  }
  return result;
}

export function redactHeaders(
  headers: Record<string, string | string[] | undefined>,
): Record<string, string> {
  const result: Record<string, string> = {};
  for (const [key, val] of Object.entries(headers)) {
    const v = Array.isArray(val) ? val.join(', ') : (val ?? '');
    const redacted = redactAttributes({ [key]: v });
    result[key] = redacted[key];
  }
  return result;
}
