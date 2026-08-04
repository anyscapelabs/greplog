// Blank stat card for the Analytics page. Intentionally empty — the Total
// Events card is designed separately from AnalyticsMetricCard and will be
// filled in later.
export default function TotalEventsCard() {
  return (
    <div
      className="min-h-28"
      style={{
        backgroundColor: 'var(--bg-secondary)',
        border: '1px solid var(--border-primary)',
        borderRadius: '10px',
      }}
    />
  )
}