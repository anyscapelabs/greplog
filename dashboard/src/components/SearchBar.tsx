import { LuSearch } from 'react-icons/lu'

interface SearchBarProps {
  value?: string
  onChange?: (value: string) => void
  /** Called when the query is submitted (Enter key). */
  onSearch?: () => void
}

function SearchBar({ value, onChange, onSearch }: SearchBarProps) {
  return (
    <div className="flex min-w-0 flex-1 items-center gap-2 rounded-md border border-zinc-700 bg-zinc-950 px-3 transition-colors focus-within:border-zinc-600 focus-within:bg-zinc-900">
      <LuSearch className="h-4 w-4 shrink-0 text-zinc-500" />
      <input
        type="text"
        placeholder="Search logs… e.g. error, level=warn, service=api-gateway, message=&quot;payment declined&quot;"
        value={value}
        onChange={(event) => onChange?.(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter') onSearch?.()
        }}
        className="h-9 w-full min-w-0 bg-transparent text-sm text-zinc-100 placeholder-zinc-500 outline-none"
      />
    </div>
  )
}

export default SearchBar