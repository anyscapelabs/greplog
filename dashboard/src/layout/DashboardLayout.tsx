import { Outlet } from 'react-router-dom'
import Sidebar from '../components/Sidebar.tsx'

export default function DashboardLayout() {
  return (
    <div className="flex h-screen">
      <Sidebar />
      <main className="flex-1 overflow-hidden" style={{ backgroundColor: 'var(--bg-primary)' }}>
        <Outlet />
      </main>
    </div>
  )
}
