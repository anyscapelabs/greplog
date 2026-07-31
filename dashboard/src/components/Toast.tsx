import { LuCircleAlert, LuCircleCheck } from 'react-icons/lu'
import type { Toast } from '../lib/toastStore.ts'
import { toastStore } from '../lib/toastStore.ts'

export function ToastViewport({ toasts }: { toasts: readonly Toast[] }) {
  return (
    <div className="toast-viewport" role="region" aria-label="Notifications" aria-live="polite">
      {toasts.map((toast) => (
        <ToastItem key={toast.id} toast={toast} />
      ))}
    </div>
  )
}

function ToastItem({ toast }: { toast: Toast }) {
  const Icon = toast.variant === 'error' ? LuCircleAlert : LuCircleCheck
  return (
    <div
      className={`toast toast-${toast.variant}`}
      style={{ backgroundColor: `var(--${toast.variant})` }}
      role={toast.variant === 'error' ? 'alert' : undefined}
    >
      <Icon className="toast-icon" aria-hidden="true" />
      <span className="toast-message">{toast.message}</span>
      <button
        type="button"
        className="toast-dismiss"
        aria-label="Dismiss notification"
        onClick={() => toastStore.dismiss(toast.id)}
      >
        ×
      </button>
      {toast.durationMs > 0 && (
        <div className="toast-progress" style={{ animationDuration: `${toast.durationMs}ms` }} aria-hidden="true" />
      )}
    </div>
  )
}
