import { LuFilter, LuX } from 'react-icons/lu'

interface SearchInputProps {
  chips: string[]
  query: string
  onQueryChange: (value: string) => void
  onKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void
  onRemoveChip: (chip: string) => void
  placeholder?: string
}

export default function SearchInput({
  chips,
  query,
  onQueryChange,
  onKeyDown,
  onRemoveChip,
  placeholder = 'Search...',
}: SearchInputProps) {
  return (
    <div className="flex-1 flex items-center gap-1.5 px-3 overflow-hidden">
      <LuFilter className="size-3.5 shrink-0" style={{ color: 'var(--text-secondary)' }} />
      <div className="flex items-center gap-1 flex-1 overflow-x-auto">
        {chips.map((chip) => (
          <span
            key={chip}
            className="flex items-center gap-1 px-2 py-0.5 text-xs text-text-primary bg-gray-100 rounded-full whitespace-nowrap shrink-0"
          >
            {chip}
            <button
              className="size-3.5 flex items-center justify-center rounded-full hover:bg-gray-200 transition-colors"
              onClick={() => onRemoveChip(chip)}
            >
              <LuX className="size-2.5" />
            </button>
          </span>
        ))}
        <input
          type="text"
          placeholder={chips.length === 0 ? placeholder : ''}
          className="flex-1 text-sm bg-transparent outline-none min-w-[120px]"
          style={{ color: 'var(--text-primary)' }}
          value={query}
          onChange={(e) => onQueryChange(e.target.value)}
          onKeyDown={onKeyDown}
        />
      </div>
    </div>
  )
}
