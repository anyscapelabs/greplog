import { LuSearch } from 'react-icons/lu'

interface SearchBarProps {
  value?: string
  onChange?: (value: string) => void
}

function SearchBar({ value, onChange }: SearchBarProps) {
  return (
    <div className="flex min-w-0 flex-1 items-center gap-2 rounded-md border border-zinc-700 bg-zinc-950 px-3 transition-colors focus-within:border-zinc-600 focus-within:bg-zinc-900">
      <LuSearch className="h-4 w-4 shrink-0 text-zinc-500" />
      <input
        type="text"
        placeholder="Search all logs"
        value={value}
        onChange={(event) => onChange?.(event.target.value)}
        className="h-9 w-full min-w-0 bg-transparent text-sm text-zinc-100 placeholder-zinc-500 outline-none"
      />
    </div>
  )
}

export default SearchBar