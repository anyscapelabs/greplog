import { useState } from 'react'
import { LuRefreshCw, LuServer } from 'react-icons/lu'
import { useAnalytics } from '../hooks/index.ts'
import { useAgent } from '../context/AgentContext.tsx'
import AnalyticsMetricCard from '../components/AnalyticsMetricCard.tsx'
import AnalyticsChartPanel from '../components/AnalyticsChartPanel.tsx'
import WaitingOverlay from '../components/WaitingOverlay.tsx'
import IngestionChart from '../components/IngestionChart.tsx'
import ErrorOverTimeChart from '../components/ErrorOverTimeChart.tsx'
import LatencyPercentilesChart from '../components/LatencyPercentilesChart.tsx'
import ServiceHealthChart from '../components/ServiceHealthChart.tsx'
import StatusCodesPieChart from '../components/StatusCodesPieChart.tsx'
import NoisyServicesChart from '../components/NoisyServicesChart.tsx'
import SeverityChart from '../components/SeverityChart.tsx'
import SystemMetricsChart from '../components/SystemMetricsChart.tsx'
import AvgResponseTimeChart from '../components/AvgResponseTimeChart.tsx'
import Dropdown from '../components/Dropdown.tsx'

export default function Analytics() {
  const { connected } = useAgent()
  const {
    metrics,
    timeRanges: timeRangeOptions,
    services: serviceOptions,
    autoRefreshOptions,
    ingestionOptions,
    rateCountOptions,
    latencyOptions,
    sortOptions,
  } = useAnalytics()

  const timeRanges = timeRangeOptions.map((r) => r.label)

  const [spinning, setSpinning] = useState(false)
  const [timeRange, setTimeRange] = useState(timeRanges[0])
  const [service, setService] = useState('All Services')
  const [autoRefresh, setAutoRefresh] = useState('Off')
  const [ingestionMetric, setIngestionMetric] = useState('sum')
  const [errorRateMetric, setErrorRateMetric] = useState('rate')
  const [latencyView, setLatencyView] = useState('p50_p90_p99')
  const [statusCodeMetric, setStatusCodeMetric] = useState('rate')
  const [noisySort, setNoisySort] = useState('logs')

  return (
    <div className="flex flex-col h-full">
      <div
        className="flex items-center px-4 h-12 shrink-0 border-b gap-3"
        style={{
          backgroundColor: 'var(--bg-secondary)',
          borderColor: 'var(--border-primary)',
        }}
      >
        <span className="text-2xl font-semibold flex items-center gap-2">
          <span style={{ color: 'var(--text-secondary)' }}>Grep</span>
          <span className="text-text-primary">Analytics</span>
        </span>
        <div className="ml-auto flex items-center gap-2">
          <button
            className="flex items-center gap-1.5 px-2 py-1 text-sm text-text-primary hover:bg-[var(--hover-bg)] transition-colors"
            style={{ borderColor: 'var(--border-primary)', borderWidth: 1 }}
            onClick={() => {
              setSpinning(true)
              setTimeout(() => setSpinning(false), 600)
            }}
          >
            <LuRefreshCw className={`size-3.5 ${spinning ? 'animate-spin' : ''}`} />
            Refresh
          </button>
          <Dropdown
            trigger={<><LuServer className="size-3.5" style={{ color: 'var(--text-secondary)' }} /><span className="text-text-primary">{service}</span></>}
            items={serviceOptions}
            value={service}
            onChange={setService}
            align="right"
            minWidth="min-w-32"
            hasBorder
          />
          <Dropdown
            trigger={<><span className="text-text-primary text-sm">Auto refresh</span>{autoRefresh !== 'Off' && <span className="flex items-center justify-center px-1.5 py-0.5 text-xs text-text-primary bg-[var(--bg-primary)] rounded">{autoRefresh}</span>}</>}
            items={autoRefreshOptions}
            value={autoRefresh}
            onChange={setAutoRefresh}
            align="right"
            minWidth="min-w-16"
            hasBorder
          />
          <Dropdown
            trigger={<span>{timeRange}</span>}
            items={timeRangeOptions}
            value={timeRange}
            onChange={setTimeRange}
            align="right"
            minWidth="min-w-40"
            hasBorder
          />
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-0.5 relative">
        <WaitingOverlay
            visible={!connected}
            message="Run the agent and configure the SDK to start collecting analytics"
            terminal={[
              '# Install the Greplog SDK',
              'npm install @greplog/sdk',
              '',
              '# Initialize the agent',
              'npx greplog init',
              '',
              '# Start collecting',
              '$ greplog agent start --endpoint http://localhost:3000',
              '',
              '# Or add to your application',
              'import { Greplog } from "@greplog/sdk"',
              'const greplog = new Greplog({ endpoint: "http://localhost:3000" })',
              'greplog.collect()',
            ]}
          />
        <div className="grid grid-cols-6 gap-0.5">
          {metrics.map((metric) => (
            <AnalyticsMetricCard key={metric.title} title={metric.title} value={metric.value} color={metric.color} rgb={metric.rgb} data={metric.sparkline} />
          ))}
        </div>
        <div className="mt-0.5">
          <AnalyticsChartPanel
            title="Log Ingestion Over Time"
            dropdownItems={ingestionOptions}
            dropdownValue={ingestionMetric}
            onDropdownChange={setIngestionMetric}
          >
            <IngestionChart />
          </AnalyticsChartPanel>
        </div>
        <div className="grid grid-cols-2 gap-0.5 mt-0.5">
          <AnalyticsChartPanel
            title="Error Rate Over Time"
            dropdownItems={rateCountOptions}
            dropdownValue={errorRateMetric}
            onDropdownChange={setErrorRateMetric}
          >
            <ErrorOverTimeChart />
          </AnalyticsChartPanel>
          <AnalyticsChartPanel
            title="Latency Percentiles"
            dropdownItems={latencyOptions}
            dropdownValue={latencyView}
            onDropdownChange={setLatencyView}
          >
            <LatencyPercentilesChart />
          </AnalyticsChartPanel>
        </div>
        <div className="grid grid-cols-3 gap-0.5 mt-0.5 pb-0.5">
          <AnalyticsChartPanel title="Service Health">
            <ServiceHealthChart />
          </AnalyticsChartPanel>
          <AnalyticsChartPanel
            title="Status Codes"
            dropdownItems={rateCountOptions}
            dropdownValue={statusCodeMetric}
            onDropdownChange={setStatusCodeMetric}
          >
            <StatusCodesPieChart />
          </AnalyticsChartPanel>
          <AnalyticsChartPanel
            title="Top Noisy Services"
            dropdownItems={sortOptions}
            dropdownValue={noisySort}
            onDropdownChange={setNoisySort}
          >
            <NoisyServicesChart />
          </AnalyticsChartPanel>
        </div>
        <div className="grid grid-cols-3 gap-0.5 mt-0.5 pb-0.5">
          <AnalyticsChartPanel title="Log Severity Distribution">
            <SeverityChart />
          </AnalyticsChartPanel>
          <AnalyticsChartPanel title="System Metrics">
            <SystemMetricsChart />
          </AnalyticsChartPanel>
          <AnalyticsChartPanel title="Avg Response Time">
            <AvgResponseTimeChart />
          </AnalyticsChartPanel>
        </div>
      </div>
    </div>
  )
}