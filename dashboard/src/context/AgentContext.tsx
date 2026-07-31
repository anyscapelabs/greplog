import { createContext, useContext, useState, useEffect, type ReactNode } from 'react'
import { fetchHealth } from '../hooks/api.ts'
import { toastStore } from '../lib/toastStore.ts'

interface AgentContextType {
  connected: boolean
}

const AgentContext = createContext<AgentContextType>({
  connected: false,
})

const AGENT_ERROR_KEY = 'agent-connection'

export function AgentProvider({ children }: { children: ReactNode }) {
  const [connected, setConnected] = useState(false)

  useEffect(() => {
    let cancelled = false
    let wasConnected = false
    let hasConnectedBefore = false

    const poll = async () => {
      while (!cancelled) {
        const health = await fetchHealth()
        const isOk = health?.status === 'ok'
        if (isOk) {
          if (!wasConnected) {
            // Recovery from a mid-session disconnect is worth a toast;
            // the initial onboarding connect is the WaitingOverlay's job.
            if (hasConnectedBefore) {
              toastStore.showSuccess('Reconnected to agent', { dedupeKey: AGENT_ERROR_KEY })
            }
            hasConnectedBefore = true
            wasConnected = true
          }
          setConnected(true)
        } else {
          // "Was connected, now isn't" is a real event worth surfacing as a
          // persistent error toast; the onboarding never-connected state is
          // already covered by the WaitingOverlay.
          if (wasConnected) {
            toastStore.showError('Agent unreachable — reconnecting…', {
              dedupeKey: AGENT_ERROR_KEY,
              durationMs: 0,
            })
          }
          wasConnected = false
          setConnected(false)
        }
        await new Promise((r) => setTimeout(r, isOk ? 5000 : 2000))
      }
    }
    poll()
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <AgentContext.Provider value={{ connected }}>
      {children}
    </AgentContext.Provider>
  )
}

// react-refresh: co-locating the provider and its consumer hook is this repo's
// established context pattern; the hook is not a component.
// eslint-disable-next-line react-refresh/only-export-components
export function useAgent() {
  return useContext(AgentContext)
}
