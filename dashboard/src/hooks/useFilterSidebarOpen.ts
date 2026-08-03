import { useState, useEffect } from 'react'
import type { Dispatch, SetStateAction } from 'react'

// Whole-panel visibility for the filter sidebar (the header "Filters" toggle).
// Shared across Logs/Errors/Services so collapsing the panel anywhere is
// remembered. Distinct from `greplog:filterSidebar:open:v3`, which tracks
// per-section collapse state inside the panel.
const STORAGE_KEY = 'greplog:filterSidebar:visible'

function loadInitial(): boolean {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (raw === null) return true
    return raw === 'true'
  } catch {
    return true
  }
}

export function useFilterSidebarOpen(): [boolean, Dispatch<SetStateAction<boolean>>] {
  const [open, setOpen] = useState<boolean>(loadInitial)

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, String(open))
    } catch {
      // storage unavailable — panel visibility persistence is best-effort
    }
  }, [open])

  return [open, setOpen]
}
