import { useCallback, useEffect, useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import Header, { type TabId, type TimeRange } from './components/Header'
import LiveTail from './pages/LiveTail'
import LogExplorer from './pages/LogExplorer'
import Metrics from './pages/Metrics'

const TAB_TITLES: Record<TabId, string> = {
  logs: 'Log Explorer · Greplog',
  metrics: 'Metrics · Greplog',
  tail: 'Live tail · Greplog',
}

/** Query-key prefixes each tab needs refreshed; the tail tab is push-fed by SSE. */
const REFRESHABLE_QUERY_KEYS: Record<TabId, string[][]> = {
  logs: [
    ['logs'],
    ['histogram'],
    ['facets'],
  ],
  metrics: [
    ['ingestion'],
    ['severity-breakdown'],
    ['ingestion-by-service'],
    ['service-table'],
    ['error-rate'],
    ['storage'],
  ],
  tail: [],
}

function getTabTitle(tab: TabId): string {
  return TAB_TITLES[tab] ?? TAB_TITLES.logs
}

function getRefreshIntervalMs(interval: string): number | null {
  if (interval === 'off') return null

  if (interval === '5s') return 5000

  if (interval === '10s') return 10000

  if (interval === '30s') return 30000

  if (interval === '1m') return 60000

  return null
}

function App() {
  const queryClient = useQueryClient()
  const [activeTab, setActiveTab] = useState<TabId>('logs')
  const [selectedRange, setSelectedRange] = useState<TimeRange>('1h')
  const [isLiveTailActive, setIsLiveTailActive] = useState(false)
  const [autoRefreshInterval, setAutoRefreshInterval] = useState('off')

  useEffect(() => {
    document.title = getTabTitle(activeTab)
  }, [activeTab])

  // Invalidate instead of remounting: queries refetch in place, so filters,
  // search text, and scroll position survive every refresh.
  const refreshActiveTab = useCallback(() => {
    if (document.visibilityState !== 'visible') return

    for (const queryKey of REFRESHABLE_QUERY_KEYS[activeTab]) {
      void queryClient.invalidateQueries({ queryKey })
    }
  }, [activeTab, queryClient])

  useEffect(() => {
    const intervalMs = getRefreshIntervalMs(autoRefreshInterval)

    if (intervalMs === null) return

    const intervalId = setInterval(refreshActiveTab, intervalMs)

    return () => clearInterval(intervalId)
  }, [autoRefreshInterval, refreshActiveTab])

  return (
    <div className="flex h-screen flex-col overflow-hidden">
      <Header
        activeTab={activeTab}
        onTabChange={setActiveTab}
        range={selectedRange}
        onRangeChange={setSelectedRange}
        liveTailActive={isLiveTailActive}
        onLiveTailToggle={() => setIsLiveTailActive((previousValue) => !previousValue)}
        refreshInterval={autoRefreshInterval}
        onRefreshIntervalChange={setAutoRefreshInterval}
        onManualRefresh={refreshActiveTab}
      />
      {activeTab === 'logs' && <LogExplorer range={selectedRange} liveTailActive={isLiveTailActive} />}
      {activeTab === 'metrics' && <Metrics range={selectedRange} />}
      {activeTab === 'tail' && <LiveTail />}
    </div>
  )
}

export default App
