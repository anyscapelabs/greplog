import { createContext, useContext, useState, useEffect, type ReactNode } from 'react'
import { fetchHealth } from '../hooks/api.ts'

interface AgentContextType {
  connected: boolean
}

const AgentContext = createContext<AgentContextType>({
  connected: false,
})

export function AgentProvider({ children }: { children: ReactNode }) {
  const [connected, setConnected] = useState(false)

  useEffect(() => {
    let cancelled = false
    const poll = async () => {
      while (!cancelled) {
        const health = await fetchHealth()
        if (health?.status === 'ok') {
          setConnected(true)
          return
        }
        await new Promise((r) => setTimeout(r, 2000))
      }
    }
    poll()
    return () => { cancelled = true }
  }, [])

  return (
    <AgentContext.Provider value={{ connected }}>
      {children}
    </AgentContext.Provider>
  )
}

export function useAgent() {
  return useContext(AgentContext)
}