import { NavLink } from 'react-router-dom'
import { CgSidebar } from 'react-icons/cg'
import { LuSearch, LuBug } from 'react-icons/lu'
import LogsIcon from '../icons/LogsIcon.tsx'
import AnalyticsIcon from '../icons/AnalyticsIcon.tsx'
import ViewsIcon from '../icons/ViewsIcon.tsx'
import ServicesIcon from '../icons/ServicesIcon.tsx'
import TracesIcon from '../icons/TracesIcon.tsx'

const primaryTabs = [
  { to: '/logs', icon: LogsIcon, label: 'Logs' },
  { to: '/analytics', icon: AnalyticsIcon, label: 'Analytics' },
  { to: '/errors', icon: LuBug, label: 'Errors' },
  { to: '/views', icon: ViewsIcon, label: 'Views' },
  { to: '/services', icon: ServicesIcon, label: 'Services' },
  { to: '/traces', icon: TracesIcon, label: 'Traces' },
]

export default function Sidebar() {
  return (
    <div
      className="flex flex-col border-r"
      style={{
        width: 'var(--sidebar-width)',
        backgroundColor: 'var(--bg-secondary)',
        borderColor: 'var(--border-primary)',
      }}
    >
      <div className="flex justify-end p-3">
        <button className="flex items-center justify-center p-2 hover:bg-gray-100 transition-colors" style={{ color: 'var(--text-primary)' }}>
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
      <nav className="flex flex-col gap-0.5 px-3">
        {primaryTabs.map((tab) => (
          <NavLink
            key={tab.to}
            to={tab.to}
            className={({ isActive }) =>
               `flex items-center gap-2.5 px-2 py-1.5 text-sm font-medium transition-colors ${
                isActive
                  ? 'text-blue-600 bg-blue-50'
                  : 'text-gray-700 hover:text-gray-900 hover:bg-gray-100'
              }`
            }
          >
            <tab.icon className="size-5" />
            {tab.label}
          </NavLink>
        ))}
      </nav>
    </div>
  )
}
