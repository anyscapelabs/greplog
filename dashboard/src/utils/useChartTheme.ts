import { useTheme } from '../context/ThemeContext.tsx'

function getCSSVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim()
}

export interface ChartThemeColors {
  grid: string
  gridStrong: string
  label: string
}

export function useChartTheme(): ChartThemeColors {
  useTheme()
  return {
    grid: getCSSVar('--chart-grid'),
    gridStrong: getCSSVar('--chart-grid-strong'),
    label: getCSSVar('--chart-label'),
  }
}

export function commonGrid(colors: ChartThemeColors) {
  return {
    axisLine: { show: false },
    axisTick: { show: false },
    splitLine: { lineStyle: { color: colors.grid, width: 1 } },
    axisLabel: { fontSize: 10, color: colors.label },
  }
}
