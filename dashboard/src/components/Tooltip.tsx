import { useState } from 'react'
import type { ReactNode } from 'react'

interface TooltipProps {
  content: string
  side?: 'top' | 'bottom' | 'bottom-left' | 'left' | 'right'
  children: ReactNode
}

const SIDE_POSITIONS: Record<
  'top' | 'bottom' | 'bottom-left' | 'left' | 'right',
  string
> = {
  top: 'bottom-full left-1/2 mb-1.5 -translate-x-1/2',
  bottom: 'top-full left-1/2 mt-1.5 -translate-x-1/2',
  'bottom-left': 'top-full right-full mt-1.5',
  left: 'right-full top-1/2 mr-1.5 -translate-y-1/2',
  right: 'left-full top-1/2 ml-1.5 -translate-y-1/2',
}

function Tooltip({ content, side = 'top', children }: TooltipProps) {
  const [visible, setVisible] = useState(false)

  return (
    <span
      className="relative flex items-center"
      onMouseEnter={() => setVisible(true)}
      onMouseLeave={() => setVisible(false)}
      onFocus={() => setVisible(true)}
      onBlur={() => setVisible(false)}
    >
      {children}
      {visible && (
        <span
          role="tooltip"
          className={`pointer-events-none absolute z-50 whitespace-nowrap rounded-md border border-zinc-700 bg-zinc-800 px-2 py-1 text-xs font-medium text-zinc-200 shadow-lg ${SIDE_POSITIONS[side]}`}
        >
          {content}
        </span>
      )}
    </span>
  )
}

export default Tooltip
