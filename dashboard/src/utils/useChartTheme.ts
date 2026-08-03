import { useMemo } from 'react'
import { useTheme } from '../context/useTheme.ts'

function getCSSVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim()
}

export interface ChartThemeColors {
  grid: string
  gridStrong: string
  label: string
  blue: string
  green: string
  orange: string
  red: string
  purple: string
}

// Memoized on `theme` so the returned object keeps a stable identity across
// renders that don't actually change the color scheme. Chart components use
// this as a dependency of imperative render effects (e.g. the uPlot-backed
// histogram) — an object that changes identity on every render would force
// those effects to tear down and rebuild the whole chart on every unrelated
// re-render (this was the root cause of a hover-tooltip flicker bug).
export function useChartTheme(): ChartThemeColors {
  const { theme } = useTheme()
  return useMemo(
    () => ({
      grid: getCSSVar('--chart-grid'),
      gridStrong: getCSSVar('--chart-grid-strong'),
      label: getCSSVar('--chart-label'),
      blue: getCSSVar('--chart-blue'),
      green: getCSSVar('--chart-green'),
      orange: getCSSVar('--chart-orange'),
      red: getCSSVar('--chart-red'),
      purple: getCSSVar('--chart-purple'),
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- CSS vars only change when `theme` toggles
    [theme],
  )
}

export function commonGrid(colors: ChartThemeColors) {
  return {
    axisLine: { show: false },
    axisTick: { show: false },
    splitLine: { lineStyle: { color: colors.grid, width: 1 } },
    axisLabel: { fontSize: 10, color: colors.label },
  }
}
