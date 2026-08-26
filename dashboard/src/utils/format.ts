const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const

export interface ByteSize {
  value: string
  unit: (typeof BYTE_UNITS)[number]
}

/**
 * Splits a byte count into a display value and unit, scaling from bytes up to
 * terabytes so small storages read "5.2 MB" instead of "0.0 GB".
 */
export function humanByteSize(bytes: number): ByteSize {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return { value: '0', unit: 'B' }
  }

  let scaled = bytes
  let unitIndex = 0
  while (scaled >= 1_000 && unitIndex < BYTE_UNITS.length - 1) {
    scaled /= 1_000
    unitIndex += 1
  }

  const decimals = unitIndex === 0 || scaled >= 100 ? 0 : 1
  return { value: scaled.toFixed(decimals), unit: BYTE_UNITS[unitIndex] }
}
