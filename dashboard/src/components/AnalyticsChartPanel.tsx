import type { ReactNode } from 'react'
import Dropdown from './Dropdown.tsx'

interface AnalyticsChartPanelProps {
  title: string
  children: ReactNode
  dropdownLabel?: string
  dropdownItems?: { label: string; value: string }[]
  dropdownValue?: string
  onDropdownChange?: (value: string) => void
  height?: string
}

export default function AnalyticsChartPanel({
  title,
  children,
  dropdownLabel,
  dropdownItems,
  dropdownValue,
  onDropdownChange,
  height = 'h-80',
}: AnalyticsChartPanelProps) {
  return (
    <div
      className={`rounded border ${height} flex flex-col`}
      style={{ backgroundColor: 'var(--bg-secondary)', borderColor: 'var(--border-primary)' }}
    >
      <div className="flex items-center gap-3 px-2 pt-2 border-b pb-2" style={{ borderColor: 'var(--border-primary)' }}>
        <span className="text-sm font-semibold text-text-primary">{title}</span>
        <div className="flex items-center gap-2 ml-auto">
          {dropdownItems && dropdownValue && onDropdownChange ? (
              <Dropdown
                trigger={<span className="text-xs text-text-secondary">{dropdownItems.find(i => i.value === dropdownValue)?.label || dropdownValue}</span>}
                items={dropdownItems}
                value={dropdownValue}
                onChange={onDropdownChange}
                minWidth="min-w-24"
                showChevron
                triggerClassName="text-xs text-text-secondary hover:text-text-primary"
              />
          ) : dropdownLabel ? (
            <button className="flex items-center gap-1 text-xs text-text-secondary hover:text-text-primary transition-colors cursor-pointer">
              {dropdownLabel}
            </button>
          ) : null}
        </div>
      </div>
      <div className="flex-1 p-1">
        {children}
      </div>
    </div>
  )
}
