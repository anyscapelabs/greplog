import { useState } from 'react'
import { useAnalytics, useFilterState, compileFilterToQuery, useRefreshControl } from '../hooks/index.ts'
import { useAgent } from '../context/AgentContext.tsx'
import PageHeader from '../components/PageHeader.tsx'
import AnalyticsMetricCard from '../components/AnalyticsMetricCard.tsx'
import AnalyticsChartPanel from '../components/AnalyticsChartPanel.tsx'
import WaitingOverlay, { SDK_SETUP_TERMINAL } from '../components/WaitingOverlay.tsx'
import IngestionChart from '../components/IngestionChart.tsx'
import ErrorOverTimeChart from '../components/ErrorOverTimeChart.tsx'
import LatencyPercentilesChart from '../components/LatencyPercentilesChart.tsx'
import ServiceHealthChart from '../components/ServiceHealthChart.tsx'
import StatusCodesPieChart from '../components/StatusCodesPieChart.tsx'
import NoisyServicesChart from '../components/NoisyServicesChart.tsx'
import SeverityChart from '../components/SeverityChart.tsx'
import SystemMetricsChart from '../components/SystemMetricsChart.tsx'
import AvgResponseTimeChart from '../components/AvgResponseTimeChart.tsx'
export default function Analytics() {
  const { connected } = useAgent()
  const {
    filters,
    setServices,
    setTimeRange,
  } = useFilterState()
  const predicate = compileFilterToQuery(filters)
  const {
    metrics,
    services: serviceOptions,
    ingestionOptions,
    rateCountOptions,
    latencyOptions,
    sortOptions,
    refetch,
    ingestionTimeseries,
    errorRateTimeseries,
    latencyData,
    serviceHealthData,
    statusCodeDistribution,
    noisyServices,
    severityDistribution,
    avgResponseTimes,
  } = useAnalytics(predicate)

  const {
    isLive,
    toggleLive,
    manualRefresh,
    autoRefresh,
    setAutoRefresh,
  } = useRefreshControl(refetch)
  const [ingestionMetric, setIngestionMetric] = useState('sum')
  const [errorRateMetric, setErrorRateMetric] = useState('rate')
  const [latencyView, setLatencyView] = useState('p50_p90_p99')
  const [statusCodeMetric, setStatusCodeMetric] = useState('rate')
  const [noisySort, setNoisySort] = useState('logs')

  const serviceLabels = serviceOptions.map((s) => s.label)
  const selectedService = filters.services.length === 1 ? filters.services[0] : 'All Services'

  function handleServiceChange(value: string) {
    if (value === 'All Services') {
      setServices([])
    } else {
      setServices([value])
    }
  }

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="Analytics"
        showLive
        isLive={isLive}
        onLiveChange={toggleLive}
        onRefresh={manualRefresh}
        showService
        timeRange={filters.timeRange}
        onTimeRangeChange={setTimeRange}
        autoRefresh={autoRefresh}
        onAutoRefreshChange={setAutoRefresh}
        services={serviceLabels}
        service={selectedService}
        onServiceChange={handleServiceChange}
      />
      <div className="flex-1 overflow-y-auto p-0.5 relative">
          <WaitingOverlay
              visible={!connected}
              message="Run the Greplog agent and configure an SDK to start collecting analytics"
              terminal={SDK_SETUP_TERMINAL}
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
            <IngestionChart data={ingestionTimeseries} />
          </AnalyticsChartPanel>
        </div>
        <div className="grid grid-cols-2 gap-0.5 mt-0.5">
          <AnalyticsChartPanel
            title="Error Rate Over Time"
            dropdownItems={rateCountOptions}
            dropdownValue={errorRateMetric}
            onDropdownChange={setErrorRateMetric}
          >
            <ErrorOverTimeChart data={errorRateTimeseries} />
          </AnalyticsChartPanel>
          <AnalyticsChartPanel
            title="Latency Percentiles"
            dropdownItems={latencyOptions}
            dropdownValue={latencyView}
            onDropdownChange={setLatencyView}
          >
            <LatencyPercentilesChart p50={latencyData.p50} p90={latencyData.p90} p99={latencyData.p99} labels={['p50', 'p90', 'p99']} />
          </AnalyticsChartPanel>
        </div>
        <div className="grid grid-cols-3 gap-0.5 mt-0.5 pb-0.5">
          <AnalyticsChartPanel title="Service Health">
            <ServiceHealthChart services={serviceHealthData} />
          </AnalyticsChartPanel>
          <AnalyticsChartPanel
            title="Status Codes"
            dropdownItems={rateCountOptions}
            dropdownValue={statusCodeMetric}
            onDropdownChange={setStatusCodeMetric}
          >
            <StatusCodesPieChart data={statusCodeDistribution} />
          </AnalyticsChartPanel>
          <AnalyticsChartPanel
            title="Top Noisy Services"
            dropdownItems={sortOptions}
            dropdownValue={noisySort}
            onDropdownChange={setNoisySort}
          >
            <NoisyServicesChart data={noisyServices.map(s => ({ label: s.name, value: s.count }))} />
          </AnalyticsChartPanel>
        </div>
        <div className="grid grid-cols-3 gap-0.5 mt-0.5 pb-0.5">
          <AnalyticsChartPanel title="Log Severity Distribution">
            <SeverityChart data={severityDistribution} />
          </AnalyticsChartPanel>
          <AnalyticsChartPanel title="System Metrics">
            <SystemMetricsChart cpu={[]} memory={[]} diskIO={[]} network={[]} />
          </AnalyticsChartPanel>
          <AnalyticsChartPanel title="Avg Response Time">
            <AvgResponseTimeChart data={avgResponseTimes.map(s => ({ label: s.service, value: s.ms }))} />
          </AnalyticsChartPanel>
        </div>
      </div>
    </div>
  )
}
