import { useEffect, useState } from 'react'
import Header, { type TabId, type TimeRange } from './components/Header'
import LiveTail from './pages/LiveTail'
import LogExplorer from './pages/LogExplorer'
import Metrics from './pages/Metrics'

const TAB_TITLES: Record<TabId, string> = {
  logs: 'Log Explorer · Greplog',
  metrics: 'Metrics · Greplog',
  tail: 'Live tail · Greplog',
}

function getTabTitle(tab: TabId): string {
  if (!tab) return TAB_TITLES.logs

  const title = TAB_TITLES[tab]

  if (!title) return TAB_TITLES.logs

  return title
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
  const [activeTab, setActiveTab] = useState<TabId>('logs')
  const [selectedRange, setSelectedRange] = useState<TimeRange>('1h')
  const [isLiveTailActive, setIsLiveTailActive] = useState(false)
  const [autoRefreshInterval, setAutoRefreshInterval] = useState('off')
  const [refreshTick, setRefreshTick] = useState(0)

  useEffect(() => {
    const nextTitle = getTabTitle(activeTab)

    document.title = nextTitle
  }, [activeTab])

  useEffect(() => {
    const intervalMs = getRefreshIntervalMs(autoRefreshInterval)

    if (intervalMs === null) return

    const intervalId = setInterval(() => setRefreshTick((previousTick) => previousTick + 1), intervalMs)

    return () => clearInterval(intervalId)
  }, [autoRefreshInterval])

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
        onManualRefresh={() => setRefreshTick((previousTick) => previousTick + 1)}
      />
      {activeTab === 'logs' && <LogExplorer key={refreshTick} range={selectedRange} liveTailActive={isLiveTailActive} />}
      {activeTab === 'metrics' && <Metrics key={refreshTick} range={selectedRange} />}
      {activeTab === 'tail' && <LiveTail />}
    </div>
  )
}

export default App
