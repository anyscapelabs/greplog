import { useState } from 'react'
import { LuRefreshCw, LuServer } from 'react-icons/lu'
import AnalyticsMetricCard from '../components/AnalyticsMetricCard.tsx'
import AnalyticsChartPanel from '../components/AnalyticsChartPanel.tsx'
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

const generateData = (base: number, variance: number, min: number = 0) => {
  let current = base;
  return Array.from({ length: 120 }, () => {
    current += (Math.random() - 0.5) * variance;
    if (current < min) current = min;
    return current;
  });
};

const metricsData = [
  { title: 'Requests', value: '1.2M', color: '#3b82f6', rgb: '59, 130, 246', data: generateData(200, 50) },
  { title: 'Error rate', value: '0.12%', color: '#dc2626', rgb: '220, 38, 38', data: generateData(1, 0.5) },
  { title: 'P.95 latency', value: '145ms', color: '#d97706', rgb: '217, 119, 6', data: generateData(145, 10) },
  { title: 'Request throughput', value: '2.4k/s', color: '#16a34a', rgb: '22, 163, 74', data: generateData(2400, 200) },
  { title: 'Active services', value: '42', color: '#2563eb', rgb: '37, 99, 235', data: generateData(42, 2) },
  { title: 'Trace volume', value: '840GB', color: '#8b5cf6', rgb: '139, 92, 246', data: generateData(800, 40) },
]

const timeRanges = ['Last 15 min', 'Last 1 hour', 'Last 6 hours', 'Last 24 hours', 'Last 7 days', 'Custom']
const services = ['All Services', 'web', 'api', 'db', 'worker']

const ingestionOptions = [
  { label: 'Sum', value: 'sum' },
  { label: 'Rate', value: 'rate' },
  { label: 'Volume', value: 'volume' },
]

const rateCountOptions = [
  { label: 'Rate', value: 'rate' },
  { label: 'Count', value: 'count' },
]

const latencyOptions = [
  { label: 'P50, P90, P99', value: 'p50_p90_p99' },
  { label: 'P50', value: 'p50' },
  { label: 'P90', value: 'p90' },
  { label: 'P99', value: 'p99' },
  { label: 'Average', value: 'avg' },
]

const sortOptions = [
  { label: 'Logs Generated', value: 'logs' },
  { label: 'Errors', value: 'errors' },
  { label: 'Latency', value: 'latency' },
]

export default function Analytics() {
  const [spinning, setSpinning] = useState(false)
  const [timeRange, setTimeRange] = useState('Last 15 min')
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
            items={services.map((s) => ({ label: s, value: s }))}
            value={service}
            onChange={setService}
            align="right"
            minWidth="min-w-32"
            hasBorder
          />
          <Dropdown
            trigger={<><span className="text-text-primary text-sm">Auto refresh</span>{autoRefresh !== 'Off' && <span className="flex items-center justify-center px-1.5 py-0.5 text-xs text-text-primary bg-[var(--bg-primary)] rounded">{autoRefresh}</span>}</>}
            items={['Off', '10s', '30s', '1m', '5m'].map((opt) => ({ label: opt, value: opt }))}
            value={autoRefresh}
            onChange={setAutoRefresh}
            align="right"
            minWidth="min-w-16"
            hasBorder
          />
          <Dropdown
            trigger={<span>{timeRange}</span>}
            items={timeRanges.map((r) => ({ label: r, value: r }))}
            value={timeRange}
            onChange={setTimeRange}
            align="right"
            minWidth="min-w-40"
            hasBorder
          />
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-0.5">
        <div className="grid grid-cols-6 gap-0.5">
          {metricsData.map((metric) => (
            <AnalyticsMetricCard key={metric.title} {...metric} />
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
