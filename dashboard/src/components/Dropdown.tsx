import { useState, useRef, useEffect, type ReactNode } from 'react'
import { LuChevronDown } from 'react-icons/lu'

interface DropdownItem {
  label: string
  value: string
}

interface DropdownProps {
  trigger: ReactNode
  items: DropdownItem[]
  value?: string
  onChange?: (value: string) => void
  align?: 'left' | 'right'
  minWidth?: string
  showChevron?: boolean
  triggerClassName?: string
  hasBorder?: boolean
}

export default function Dropdown({
  trigger,
  items,
  value,
  onChange,
  align = 'left',
  minWidth = 'min-w-32',
  showChevron = true,
  triggerClassName = '',
  hasBorder = false,
}: DropdownProps) {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false)
      }
    }
    if (open) {
      document.addEventListener('mousedown', handleClickOutside)
    }
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [open])

  return (
    <div className="relative" ref={ref}>
      <button
        className={`flex items-center gap-1.5 transition-colors ${triggerClassName || 'px-2 py-1 text-sm text-text-primary hover:bg-gray-100'}`}
        style={hasBorder ? { borderColor: 'var(--border-primary)', borderWidth: 1 } : undefined}
        onClick={() => setOpen(!open)}
      >
        {trigger}
        {showChevron && <LuChevronDown className="size-3" style={{ color: 'var(--text-secondary)' }} />}
      </button>
      {open && (
        <div
          className={`absolute top-full ${align === 'right' ? 'right-0' : 'left-0'} mt-1 py-1 ${minWidth} rounded border bg-white shadow-md z-[100]`}
          style={{ borderColor: 'var(--border-primary)' }}
        >
          {items.map((item) => (
            <button
              key={item.value}
              className={`w-full text-left px-3 py-1.5 text-sm transition-colors ${
                item.value === value
                  ? 'text-text-primary bg-gray-100 font-medium'
                  : 'text-text-primary hover:bg-gray-50'
              }`}
              onClick={() => {
                onChange?.(item.value)
                setOpen(false)
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
