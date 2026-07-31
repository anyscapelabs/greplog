import { createContext, useMemo, useSyncExternalStore, useContext, type ReactNode } from 'react'
import { toastStore, type ToastOpts } from '../lib/toastStore.ts'
import { ToastViewport } from '../components/Toast.tsx'

interface ToastApi {
  showError: (message: string, opts?: ToastOpts) => void
  showSuccess: (message: string, opts?: ToastOpts) => void
}

const ToastContext = createContext<ToastApi | null>(null)

export function ToastProvider({ children }: { children: ReactNode }) {
  const toasts = useSyncExternalStore(toastStore.subscribe, toastStore.getToasts)
  const api = useMemo<ToastApi>(
    () => ({
      showError: toastStore.showError,
      showSuccess: toastStore.showSuccess,
    }),
    [],
  )

  return (
    <ToastContext.Provider value={api}>
      {children}
      <ToastViewport toasts={toasts} />
    </ToastContext.Provider>
  )
}

export function useToast(): ToastApi {
  const api = useContext(ToastContext)
  if (!api) {
    throw new Error('useToast must be used within a ToastProvider')
  }
  return api
}
