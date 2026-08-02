import { useContext } from 'react'
import { ThemeContext } from './theme-context.ts'

export function useTheme() {
  return useContext(ThemeContext)
}
