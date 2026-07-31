import type { ReactNode } from 'react'
import { LuX } from 'react-icons/lu'

interface DrawerProps {
  open: boolean
  onClose: () => void
  title: ReactNode
  children: ReactNode
  width?: string
}

export default function Drawer({ open, onClose, title, children, width = '900px' }: DrawerProps) {
  if (!open) return null

  return (
    <>
      <div className="fixed inset-0 bg-black/30 z-40" onClick={onClose} />
      <div
        className="fixed top-0 right-0 h-full border-l shadow-xl z-50 flex flex-col animate-in slide-in-from-right"
        style={{
          width,
          backgroundColor: 'var(--bg-secondary)',
          borderColor: 'var(--border-primary)',
        }}
      >
        <div className="flex items-center justify-between px-4 h-12 border-b shrink-0" style={{ borderColor: 'var(--border-primary)' }}>
          <span className="text-sm font-semibold text-text-primary">{title}</span>
          <button
            className="flex items-center justify-center size-7 rounded hover:bg-[var(--hover-bg)] transition-colors"
            onClick={onClose}
          >
            <LuX className="size-4" style={{ color: 'var(--text-secondary)' }} />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto p-4">
          {children}
        </div>
      </div>
    </>
  )
}
