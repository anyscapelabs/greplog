import { IoInformationCircleOutline } from 'react-icons/io5'
import Tooltip from '../Tooltip'

type Props = {
  valueGb?: number | null
  isLoading?: boolean
  isError?: boolean
  errorMessage?: string
}

export default function Storage({ valueGb = null, isLoading = false, isError = false, errorMessage }: Props) {
  if (isError) {
    return (
      <div className="flex min-h-0 flex-1 flex-col rounded-lg border border-red-800 bg-red-950/20 p-4">
        <p className="text-sm font-medium text-red-400">Failed to load storage</p>
        <p className="mt-1 text-xs text-zinc-400">{errorMessage ?? 'Unknown error'}</p>
      </div>
    )
  }

  const hasData = valueGb !== null && Number.isFinite(valueGb)

  return (
    <div className="flex min-h-0 flex-1 flex-col rounded-lg border border-zinc-800">
      <div className="flex items-center gap-1.5 border-b border-zinc-800 px-3 py-2">
        <h2 className="text-xs font-medium uppercase tracking-wide text-zinc-100">Storage</h2>

        <Tooltip
          side="bottom-start"
          content="Disk usage and retention status — Parquet storage size, partition count and TTL remaining."
        >
          <span className="cursor-pointer rounded p-1 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300">
            <IoInformationCircleOutline className="h-4 w-4" />
          </span>
        </Tooltip>

        {isLoading && <span className="ml-auto text-xs text-zinc-500">loading…</span>}
      </div>

      <div className="flex flex-1 items-center justify-center gap-1.5 overflow-hidden rounded-b-lg bg-[#16a34a] p-4">
        {!hasData && !isLoading && <span className="text-sm font-medium text-white/90">No data</span>}

        {hasData && (
          <>
            <span className="font-mono text-8xl font-bold tracking-tight text-white">{valueGb!.toFixed(1)}</span>
            <span className="pb-1 font-mono text-sm font-medium text-white/90">GB</span>
          </>
        )}

        {isLoading && <span className="font-mono text-sm text-white/70">loading…</span>}
      </div>
    </div>
  )
}
