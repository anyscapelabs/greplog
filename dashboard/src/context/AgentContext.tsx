import { createContext, useContext, useState, useCallback, type ReactNode } from 'react'

interface AgentContextType {
  connected: boolean
  setConnected: () => void
}

const AgentContext = createContext<AgentContextType>({
  connected: false,
  setConnected: () => {},
})

export function AgentProvider({ children }: { children: ReactNode }) {
  const [connected, setConnectedState] = useState(false)
  const setConnected = useCallback(() => setConnectedState(true), [])

  return (
    <AgentContext.Provider value={{ connected, setConnected }}>
      {children}
    </AgentContext.Provider>
  )
}

export function useAgent() {
  return useContext(AgentContext)
}