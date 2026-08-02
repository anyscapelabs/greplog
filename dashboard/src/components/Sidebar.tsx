import { NavLink } from 'react-router-dom'
import { CgSidebar } from 'react-icons/cg'
import { LuSearch, LuBug, LuSun, LuMoon } from 'react-icons/lu'
import LogsIcon from '../icons/LogsIcon.tsx'
import AnalyticsIcon from '../icons/AnalyticsIcon.tsx'
import ServicesIcon from '../icons/ServicesIcon.tsx'
import { useTheme } from '../context/ThemeContext.tsx'

const allTabs = [
  { to: '/logs', icon: LogsIcon, label: 'Logs' },
  { to: '/analytics', icon: AnalyticsIcon, label: 'Analytics' },
  { to: '/errors', icon: LuBug, label: 'Errors' },
  { to: '/services', icon: ServicesIcon, label: 'Services' },
]

export default function Sidebar() {
  const { theme, toggleTheme } = useTheme()

  return (
    <div
      className="flex flex-col border-r"
      style={{
        width: 'var(--sidebar-width)',
        backgroundColor: 'var(--bg-secondary)',
        borderColor: 'var(--border-primary)',
      }}
    >
      <div className="flex items-center justify-between p-3">
        <img
          src={theme === 'dark' ? '/wordmark-white.svg' : '/wordmark-black.svg'}
          alt="Greplog"
          className="h-5"
        />
        <button className="flex items-center justify-center p-2 hover:bg-[var(--hover-bg)] transition-colors" style={{ color: 'var(--text-primary)' }}>
          <CgSidebar className="size-5" />
        </button>
      </div>
      <div className="px-3 pb-3">
        <div
          className="flex items-center px-2 py-1 focus-within:ring-2 focus-within:ring-gray-400"
          style={{
            borderColor: 'var(--border-primary)',
            borderWidth: 1,
          }}
        >
          <LuSearch className="size-4 shrink-0 mr-1.5" style={{ color: 'var(--text-secondary)' }} />
          <input
            type="text"
            placeholder="Search"
            className="flex-1 text-sm bg-transparent outline-none"
            style={{ color: 'var(--text-primary)' }}
          />
        </div>
      </div>
      <nav className="flex flex-col gap-0.5 px-3 flex-1">
        {allTabs.map((tab) => (
          <NavLink
            key={tab.to}
            to={tab.to}
            className={({ isActive }) =>
              `flex items-center gap-2.5 px-2 py-1.5 text-sm font-medium transition-colors ${
              isActive
                ? 'text-[var(--accent)] bg-[var(--accent)]/10'
                : 'text-text-secondary hover:text-text-primary hover:bg-[var(--hover-bg)]'
            }`
            }
          >
            <tab.icon className="size-5" />
            {tab.label}
          </NavLink>
        ))}
      </nav>
      <div className="px-3 pb-3">
        <button
          onClick={toggleTheme}
          className="flex items-center gap-2.5 w-full px-2 py-1.5 text-sm text-text-secondary hover:text-text-primary hover:bg-[var(--hover-bg)] transition-colors rounded"
        >
          {theme === 'dark' ? <LuSun className="size-5" /> : <LuMoon className="size-5" />}
          <span>{theme === 'dark' ? 'Light Mode' : 'Dark Mode'}</span>
        </button>
      </div>
    </div>
  )
}