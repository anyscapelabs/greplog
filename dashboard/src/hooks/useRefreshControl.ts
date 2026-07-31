import { useState, useEffect, useCallback, useMemo } from 'react'

interface UseRefreshControlOptions {
  defaultLiveIntervalMs?: number
  /**
   * Optional manual-refresh callback. When provided, `manualRefresh` calls it
   * instead of the plain background `refetch` (used to mark the next query as
   * user-initiated so toast anti-spam rules don't swallow the feedback).
   */
  manualRefetch?: () => void
}

export function useRefreshControl(refetch: () => void, opts?: UseRefreshControlOptions) {
  const [isLive, setIsLive] = useState(false)
  const [autoRefresh, setAutoRefresh] = useState('Off')
  const { defaultLiveIntervalMs, manualRefetch } = opts ?? {}

  const intervalMs = useMemo(() => {
    if (isLive) return defaultLiveIntervalMs ?? 5000
    if (autoRefresh === 'Off') return null
    const m = autoRefresh.match(/^(\d+)([sm])$/)
    if (!m) return null
    const n = parseInt(m[1])
    return m[2] === 'm' ? n * 60000 : n * 1000
  }, [isLive, autoRefresh, defaultLiveIntervalMs])

  useEffect(() => {
    if (intervalMs === null) return
    const id = setInterval(() => refetch(), intervalMs)
    return () => clearInterval(id)
  }, [intervalMs, refetch])

  const manualRefresh = useCallback(() => {
    if (manualRefetch) manualRefetch()
    else refetch()
  }, [refetch, manualRefetch])
  const toggleLive = useCallback((value?: boolean) => setIsLive(v => value ?? !v), [])

  return { isLive, toggleLive, manualRefresh, autoRefresh, setAutoRefresh }
}
