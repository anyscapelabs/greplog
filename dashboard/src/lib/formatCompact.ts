export function formatCompact(value: number): string {
  const abs = Math.abs(value)
  let scaled: number
  let suffix: string
  if (abs >= 1e9) {
    scaled = value / 1e9
    suffix = 'B'
  } else if (abs >= 1e6) {
    scaled = value / 1e6
    suffix = 'M'
  } else if (abs >= 1e3) {
    scaled = value / 1e3
    suffix = 'K'
  } else {
    return String(value)
  }
  const trimmed = scaled.toFixed(2).replace(/\.?0+$/, '')
  return `${trimmed}${suffix}`
}
