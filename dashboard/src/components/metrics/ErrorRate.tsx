import { IoInformationCircleOutline } from 'react-icons/io5'
import Tooltip from '../Tooltip'
import Spinner from '../Spinner'

type Props = {
  value?: number | null
  isLoading?: boolean
  isError?: boolean
  errorMessage?: string
}

export default function ErrorRate({ value = null, isLoading = false, isError = false, errorMessage }: Props) {
  if (isError) {
    return (
      <div className="flex min-h-0 flex-1 flex-col rounded-lg border border-red-800 bg-red-950/20 p-4">
        <p className="text-sm font-medium text-red-400">Failed to load error rate</p>
        <p className="mt-1 text-xs text-zinc-400">{errorMessage ?? 'Unknown error'}</p>
      </div>
    )
  }

  const hasData = value !== null && Number.isFinite(value)

  return (
    <div className="flex min-h-0 flex-1 flex-col rounded-lg border border-zinc-800">
      <div className="flex items-center gap-1.5 border-b border-zinc-800 px-3 py-2">
        <h2 className="text-xs font-medium uppercase tracking-wide text-zinc-100">Error Rate</h2>

        <Tooltip
          side="bottom-start"
          content="Percentage of logs with level ERROR out of total logs per bucket over the selected time range."
        >
          <span className="cursor-pointer rounded p-1 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-300">
            <IoInformationCircleOutline className="h-4 w-4" />
          </span>
        </Tooltip>
      </div>

      <div className="flex flex-1 items-center justify-center overflow-hidden rounded-b-lg bg-[#dc2626] p-4">
        {!hasData && !isLoading && <span className="text-sm font-medium text-white/90">No data</span>}

        {hasData && (
          <>
            <span className="font-mono text-8xl font-bold tracking-tight text-white">{value!.toFixed(1)}</span>
            <span className="pb-1 font-mono text-sm font-medium text-white/90">%</span>
          </>
        )}

        {isLoading && !hasData && <Spinner tone="light" className="h-8 w-8" />}
      </div>
    </div>
  )
}
