import {
  placeholderTimeRanges,
  placeholderAutoRefreshOptions,
  placeholderServices,
  placeholderChartMetrics,
  placeholderCountRateOptions,
  placeholderLatencyOptions,
} from './placeholder-data.ts'
import type { DropdownOption } from '../types/index.ts'

interface SharedDropdowns {
  timeRanges: DropdownOption[]
  autoRefreshOptions: DropdownOption[]
  services: DropdownOption[]
  chartMetrics: DropdownOption[]
  countRateOptions: DropdownOption[]
  latencyOptions: DropdownOption[]
}

const shared: SharedDropdowns = {
  timeRanges: placeholderTimeRanges,
  autoRefreshOptions: placeholderAutoRefreshOptions,
  services: placeholderServices,
  chartMetrics: placeholderChartMetrics,
  countRateOptions: placeholderCountRateOptions,
  latencyOptions: placeholderLatencyOptions,
}

export function useSharedDropdowns(): SharedDropdowns {
  return shared
}