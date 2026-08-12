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

function App() {
  const [tab, setTab] = useState<TabId>('logs')
  const [range, setRange] = useState<TimeRange>('1h')

  useEffect(() => {
    document.title = TAB_TITLES[tab]
  }, [tab])

  return (
    <div className="flex h-screen flex-col overflow-hidden">
      <Header
        activeTab={tab}
        onTabChange={setTab}
        range={range}
        onRangeChange={setRange}
      />
      {tab === 'logs' && <LogExplorer range={range} />}
      {tab === 'metrics' && <Metrics />}
      {tab === 'tail' && <LiveTail />}
    </div>
  )
}

export default App