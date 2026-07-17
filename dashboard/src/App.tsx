import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import DashboardLayout from './layout/DashboardLayout.tsx'
import Logs from './pages/Logs.tsx'
import Analytics from './pages/Analytics.tsx'
import Errors from './pages/Errors.tsx'
import Services from './pages/Services.tsx'
import Patterns from './pages/Patterns.tsx'

function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<DashboardLayout />}>
          <Route index element={<Navigate to="/logs" replace />} />
          <Route path="logs" element={<Logs />} />
          <Route path="analytics" element={<Analytics />} />
          <Route path="errors" element={<Errors />} />
          <Route path="services" element={<Services />} />
          <Route path="patterns" element={<Patterns />} />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}

export default App
