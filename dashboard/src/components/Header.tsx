import wordmark from '../assets/wordmark-white.svg'

function Header() {
  return (
    <header className="flex items-center justify-between border-b border-zinc-800 px-3 py-2">
      <img src={wordmark} alt="Greplog" className="h-5 w-auto" />
    </header>
  )
}

export default Header