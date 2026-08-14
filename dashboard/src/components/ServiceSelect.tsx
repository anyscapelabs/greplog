import { useState } from 'react'
import { LuChevronDown, LuChevronUp } from 'react-icons/lu'
import ServiceIcon from './icons/ServiceIcon'

interface ServiceSelectProps {
  /** Distinct service names fetched from storage; "All services" is prepended. */
  services: string[]
  value: string
  onChange: (service: string) => void
}

function ServiceSelect({ services, value, onChange }: ServiceSelectProps) {
  const [open, setOpen] = useState(false)

  const options = ['All services', ...services]

  const isAll = value === 'All services'

  return (
    <div className="relative inline-block">
      <button
        type="button"
        onClick={() => setOpen((current) => !current)}
        className="flex h-9 w-40 cursor-pointer items-center gap-2 rounded-md border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-300 transition-colors hover:border-zinc-600 hover:text-zinc-100"
      >
        <span className="text-zinc-400">
          <ServiceIcon type={isAll ? 'box' : 'database'} />
        </span>
        <span className="truncate font-medium">{value}</span>
        {open ? (
          <LuChevronUp className="h-4 w-4 shrink-0 text-zinc-500" />
        ) : (
          <LuChevronDown className="h-4 w-4 shrink-0 text-zinc-500" />
        )}
      </button>
      {open && (
        <>
          <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
          <ul className="absolute left-0 top-full z-20 mt-1 max-h-64 w-full overflow-y-auto rounded-md border border-zinc-700 bg-zinc-900 py-1 text-sm shadow-lg">
            {options.map((service) => (
              <li key={service}>
                <button
                  type="button"
                  onClick={() => {
                    onChange(service)
                    setOpen(false)
                  }}
                  className={`flex w-full cursor-pointer items-center gap-2 px-3 py-1.5 text-left transition-colors hover:bg-zinc-800 ${
                    value === service
                      ? 'font-medium text-zinc-100'
                      : 'text-zinc-400'
                  }`}
                >
                  <span
                    className={`shrink-0 ${
                      value === service ? 'text-zinc-200' : 'text-zinc-500'
                    }`}
                  >
                    <ServiceIcon
                      type={service === 'All services' ? 'box' : 'database'}
                    />
                  </span>
                  <span className="truncate">{service}</span>
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  )
}

export default ServiceSelect