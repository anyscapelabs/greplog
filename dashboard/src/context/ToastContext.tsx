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

// react-refresh: co-locating the provider and its consumer hook is this repo's
// established context pattern (see AgentContext/ThemeContext); the hook is not
// a component and the provider gains nothing from fast-refresh replacement.
// eslint-disable-next-line react-refresh/only-export-components
export function useToast(): ToastApi {
  const api = useContext(ToastContext)
  if (!api) {
    throw new Error('useToast must be used within a ToastProvider')
  }
  return api
}
