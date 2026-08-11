import wordmark from '../assets/wordmark-white.svg'

function Header() {
  return (
    <header className="flex items-center justify-between border-b border-zinc-800 px-6 py-4">
      <img src={wordmark} alt="Greplog" className="h-6 w-auto" />
    </header>
  )
}

export default Header