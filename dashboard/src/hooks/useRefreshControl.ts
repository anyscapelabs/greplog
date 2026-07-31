import { useState, useEffect, useCallback, useMemo } from 'react'

interface UseRefreshControlOptions {
  defaultLiveIntervalMs?: number
}

export function useRefreshControl(refetch: () => void, opts?: UseRefreshControlOptions) {
  const [isLive, setIsLive] = useState(false)
  const [autoRefresh, setAutoRefresh] = useState('Off')

  const intervalMs = useMemo(() => {
    if (isLive) return opts?.defaultLiveIntervalMs ?? 5000
    if (autoRefresh === 'Off') return null
    const m = autoRefresh.match(/^(\d+)([sm])$/)
    if (!m) return null
    const n = parseInt(m[1])
    return m[2] === 'm' ? n * 60000 : n * 1000
  }, [isLive, autoRefresh, opts?.defaultLiveIntervalMs])

  useEffect(() => {
    if (intervalMs === null) return
    const id = setInterval(() => refetch(), intervalMs)
    return () => clearInterval(id)
  }, [intervalMs, refetch])

  const manualRefresh = useCallback(() => { refetch() }, [refetch])
  const toggleLive = useCallback((value?: boolean) => setIsLive(v => value ?? !v), [])

  return { isLive, toggleLive, manualRefresh, autoRefresh, setAutoRefresh }
}
