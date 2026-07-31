/**
 * Toast notification store.
 *
 * Module-level singleton (exported as `toastStore`) so plain non-React code
 * (`hooks/api.ts`, `context/AgentContext.tsx`) can surface toasts without a
 * component. React renders it via `useSyncExternalStore` in ToastProvider.
 *
 * Anti-spam rules (AGENTS.md-relevant design requirement — the failure mode
 * is a wall of red toasts stacking on every auto-refresh poll):
 *   - `dedupeKey` present  => background-style failure: dedupe by key (never
 *     two identical toasts at once) AND rate-limit repeats to at most one per
 *     ERROR_COOLDOWN_MS while the failure persists.
 *   - `dedupeKey` absent   => user-initiated action failure: always shown, no
 *     dedupe suppression (the user took an action and deserves the feedback).
 *   - Recovery: `showSuccess(..., { dedupeKey })` only surfaces when that key
 *     is currently in an error state, closing the loop with a "reconnected"
 *     toast instead of the error silently expiring.
 */
export type ToastVariant = 'error' | 'success'

export interface Toast {
  id: string
  variant: ToastVariant
  message: string
  dedupeKey?: string
  /** Auto-dismiss delay in ms. 0 = never auto-dismiss (manual dismissal only). */
  durationMs: number
}

export interface ToastOpts {
  /** Background-style failures: dedupe + rate-limit identical repeats. */
  dedupeKey?: string
  /**
   * User-initiated action failure: bypass dedupe + cooldown entirely and show
   * unconditionally (the user took an action and deserves the feedback). Such
   * toasts carry no `dedupeKey`, so they never disturb background key-state.
   */
  userInitiated?: boolean
  /** Override the per-variant default duration. 0 = persistent. */
  durationMs?: number
}

export interface ToastStore {
  subscribe: (listener: () => void) => () => void
  getToasts: () => readonly Toast[]
  showError: (message: string, opts?: ToastOpts) => void
  showSuccess: (message: string, opts?: ToastOpts) => void
  dismiss: (id: string) => void
}

export interface ToastStoreDeps {
  now?: () => number
  schedule?: (fn: () => void, ms: number) => number
  cancel?: (timerId: number) => void
}

export const DEFAULT_ERROR_DURATION_MS = 8_000
export const DEFAULT_SUCCESS_DURATION_MS = 4_000
export const ERROR_COOLDOWN_MS = 60_000

interface KeyState {
  lastShownAt: number
  inError: boolean
}

export function createToastStore(deps: ToastStoreDeps = {}): ToastStore {
  const now = deps.now ?? Date.now
  const schedule = deps.schedule ?? ((fn: () => void, ms: number) => window.setTimeout(fn, ms))
  const cancel = deps.cancel ?? ((timerId: number) => window.clearTimeout(timerId))

  let toasts: Toast[] = []
  let nextId = 0
  const listeners = new Set<() => void>()
  const timers = new Map<string, number>()
  const keyState = new Map<string, KeyState>()

  const notify = () => {
    for (const listener of listeners) listener()
  }

  const dismissToast = (id: string) => {
    const timerId = timers.get(id)
    if (timerId !== undefined) {
      cancel(timerId)
      timers.delete(id)
    }
    const next = toasts.filter((t) => t.id !== id)
    if (next.length !== toasts.length) {
      toasts = next
      notify()
    }
  }

  const dismissByKey = (dedupeKey: string) => {
    for (const t of toasts) {
      if (t.dedupeKey === dedupeKey) dismissToast(t.id)
    }
  }

  const pushToast = (variant: ToastVariant, message: string, dedupeKey: string | undefined, durationMs: number) => {
    const id = String(++nextId)
    const toast: Toast = { id, variant, message, dedupeKey, durationMs }
    toasts = [...toasts, toast]
    notify()
    if (durationMs > 0) {
      timers.set(id, schedule(() => dismissToast(id), durationMs))
    }
  }

  const showError = (message: string, opts?: ToastOpts) => {
    const { dedupeKey, durationMs, userInitiated } = opts ?? {}
    // User-initiated actions bypass all anti-spam rules and never carry a
    // dedupeKey, so they cannot collide with or reset background key-state.
    if (userInitiated) {
      pushToast('error', message, undefined, durationMs ?? DEFAULT_ERROR_DURATION_MS)
      return
    }
    if (dedupeKey) {
      if (toasts.some((t) => t.dedupeKey === dedupeKey)) {
        console.log(`[toast] suppressed duplicate error (${dedupeKey}) — already showing`)
        return
      }
      const state = keyState.get(dedupeKey)
      if (state && now() - state.lastShownAt < ERROR_COOLDOWN_MS) {
        console.log(
          `[toast] suppressed error (${dedupeKey}) — shown ${now() - state.lastShownAt}ms ago, within ${ERROR_COOLDOWN_MS}ms cooldown`,
        )
        return
      }
      keyState.set(dedupeKey, { lastShownAt: now(), inError: true })
    }
    pushToast('error', message, dedupeKey, durationMs ?? DEFAULT_ERROR_DURATION_MS)
  }

  const showSuccess = (message: string, opts?: ToastOpts) => {
    const { dedupeKey, durationMs } = opts ?? {}
    if (dedupeKey) {
      const state = keyState.get(dedupeKey)
      if (!state?.inError) return
      dismissByKey(dedupeKey)
      // Forget the key entirely: the next failure is a fresh event, not a
      // rate-limited repeat of the old one (no stale cooldown).
      keyState.delete(dedupeKey)
    }
    pushToast('success', message, dedupeKey, durationMs ?? DEFAULT_SUCCESS_DURATION_MS)
  }

  return {
    subscribe: (listener: () => void) => {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    getToasts: () => toasts,
    showError,
    showSuccess,
    dismiss: dismissToast,
  }
}

export const toastStore = createToastStore()
