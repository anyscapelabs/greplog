/** Visual identity for one log severity, shared by rows, bars and charts. */
export interface SeverityStyle {
  /** Tailwind text class for the level label in log rows. */
  text: string
  /** Chart stroke / bar outline color. */
  stroke: string
  /** Translucent chart fill paired with `stroke`. */
  fill: string
}

/**
 * The severity palette. Levels are free-form strings end to end (the engine
 * stores them verbatim), so anything missing here falls back via
 * `severityStyle`; UNKNOWN covers empty/unparsed labels.
 */
const SEVERITY_STYLES: Record<string, SeverityStyle> = {
  TRACE: { text: 'text-violet-400', stroke: '#a78bfa', fill: 'rgba(167, 139, 250, 0.55)' },
  DEBUG: { text: 'text-zinc-500', stroke: '#71717a', fill: 'rgba(161, 161, 170, 0.6)' },
  INFO: { text: 'text-sky-400', stroke: '#38bdf8', fill: 'rgba(56, 189, 248, 0.6)' },
  WARN: { text: 'text-amber-400', stroke: '#fbbf24', fill: 'rgba(251, 191, 36, 0.6)' },
  ERROR: { text: 'text-red-400', stroke: '#f87171', fill: 'rgba(248, 113, 113, 0.6)' },
  CRITICAL: { text: 'text-fuchsia-400', stroke: '#e879f9', fill: 'rgba(232, 121, 249, 0.6)' },
  FATAL: { text: 'text-rose-400', stroke: '#fb7185', fill: 'rgba(251, 113, 133, 0.6)' },
  UNKNOWN: { text: 'text-zinc-400', stroke: '#a1a1aa', fill: 'rgba(161, 161, 170, 0.45)' },
}

/** Fixed order for stacked/separated severity series in charts. */
export const SEVERITY_ORDER = [
  'TRACE',
  'DEBUG',
  'INFO',
  'WARN',
  'ERROR',
  'CRITICAL',
  'FATAL',
  'UNKNOWN',
]

const DEFAULT_STYLE: SeverityStyle = {
  text: 'text-zinc-400',
  stroke: '#8ab4f8',
  fill: 'rgba(138, 180, 248, 0.5)',
}

/** Uppercases a raw level value so lookups are case-insensitive. */
export function normalizeLevel(value: unknown): string {
  return String(value ?? '')
    .trim()
    .toUpperCase()
}

export function severityStyle(level?: string | null): SeverityStyle {
  if (!level) return DEFAULT_STYLE

  return SEVERITY_STYLES[normalizeLevel(level)] ?? DEFAULT_STYLE
}
