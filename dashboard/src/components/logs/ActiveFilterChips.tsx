import { LuX } from 'react-icons/lu'

interface ActiveFilterChipsProps {
  /** Effective facets currently applied, wire-named ({ level, service }). */
  facets: Record<string, string>
  /** Active free-text search, if any. */
  search?: string
  onRemoveFacet: (key: string) => void
  onRemoveSearch?: () => void
  onClearAll?: () => void
}

/**
 * Every filter currently shaping the view, each removable in place. Shown
 * whenever anything is active so state is never invisible.
 */
function ActiveFilterChips({
  facets,
  search,
  onRemoveFacet,
  onRemoveSearch,
  onClearAll,
}: ActiveFilterChipsProps) {
  const facetEntries = Object.entries(facets).filter(([, value]) => value)
  const hasSearch = Boolean(search)

  if (facetEntries.length === 0 && !hasSearch) return null

  const removableCount =
    facetEntries.length + (hasSearch && onRemoveSearch ? 1 : 0)
  const showClearAll = onClearAll && removableCount > 1

  return (
    <div className="flex flex-wrap items-center gap-1.5 border-b border-zinc-800 px-3 py-1.5">
      <span className="mr-1 text-[10px] font-medium uppercase tracking-wide text-zinc-500">
        filters
      </span>

      {facetEntries.map(([key, value]) => (
        <span
          key={key}
          className="flex items-center gap-1 rounded-full border border-blue-900 bg-blue-950/40 px-2 py-0.5 text-xs text-zinc-100"
        >
          <span className="text-blue-400">{key}:</span>
          {value}
          <button
            type="button"
            onClick={() => onRemoveFacet(key)}
            aria-label={`Remove ${key} filter`}
            className="cursor-pointer text-zinc-400 transition-colors hover:text-white"
          >
            <LuX className="h-3 w-3" />
          </button>
        </span>
      ))}

      {hasSearch && onRemoveSearch && (
        <span className="flex items-center gap-1 rounded-full border border-zinc-700 bg-zinc-800 px-2 py-0.5 text-xs text-zinc-100">
          <span className="text-zinc-400">search:</span>
          {search}
          <button
            type="button"
            onClick={onRemoveSearch}
            aria-label="Remove search"
            className="cursor-pointer text-zinc-400 transition-colors hover:text-white"
          >
            <LuX className="h-3 w-3" />
          </button>
        </span>
      )}

      {showClearAll && (
        <button
          type="button"
          onClick={onClearAll}
          className="ml-1 cursor-pointer text-xs text-zinc-400 underline-offset-2 transition-colors hover:text-zinc-200 hover:underline"
        >
          clear all
        </button>
      )}
    </div>
  )
}

export default ActiveFilterChips
