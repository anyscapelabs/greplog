// Slim animated bar pinned to the top edge of a `relative` positioned
// container, sweeping left-to-right on a loop. Used to indicate a query is
// in flight (initial load, filter/time-range change, or manual refresh)
// without blocking the rest of the card's content the way a full overlay
// spinner would.
export default function TopLoadingBar({ active }: { active: boolean }) {
  if (!active) return null
  return <div className="top-loading-bar" aria-hidden="true" />
}
