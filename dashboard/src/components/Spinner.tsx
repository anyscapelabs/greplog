import { LuLoaderCircle } from 'react-icons/lu'

interface SpinnerProps {
  className?: string
  /** "grey" fits dark surfaces; "light" reads on saturated colored tiles. */
  tone?: 'grey' | 'light'
}

/** Shared loading indicator — a spinner, never a text label. */
function Spinner({ className = 'h-5 w-5', tone = 'grey' }: SpinnerProps) {
  const toneClass = tone === 'light' ? 'text-white/80' : 'text-zinc-500'
  return <LuLoaderCircle className={`animate-spin ${toneClass} ${className}`} />
}

export default Spinner
