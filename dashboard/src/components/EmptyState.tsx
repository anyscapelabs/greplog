import type { ReactNode } from 'react'
import EmptyIcon from './icons/EmptyIcon'

interface EmptyStateProps {
  /** Short headline, e.g. "No logs found". */
  title: string
  /** Optional supporting line under the title. */
  description?: ReactNode
  /** "md" fills lists and tables; "sm" fits inside chart cards. */
  size?: 'sm' | 'md'
}

/** Shared empty state so every surface that can run dry reads the same way. */
function EmptyState({ title, description, size = 'md' }: EmptyStateProps) {
  const isCompact = size === 'sm'

  return (
    <div
      className={`flex flex-col items-center justify-center px-6 text-center ${
        isCompact ? 'gap-1.5' : 'gap-3'
      }`}
    >
      <EmptyIcon className={isCompact ? 'h-9 w-9 text-zinc-600' : 'h-14 w-14 text-zinc-600'} />
      <div>
        <p className={`font-medium text-zinc-100 ${isCompact ? 'text-sm' : 'text-base'}`}>
          {title}
        </p>
        {description && (
          <p
            className={`mt-0.5 font-medium text-zinc-400 ${isCompact ? 'text-xs' : 'text-sm'}`}
          >
            {description}
          </p>
        )}
      </div>
    </div>
  )
}

export default EmptyState
