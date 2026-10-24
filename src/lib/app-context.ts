import { createContext, useContext } from 'react'
import type { AppInfo, Settings } from '@/lib/api'

export type AppContext = {
  info: AppInfo
  settings: Settings
  saveSettings: (patch: Partial<Settings>) => Promise<void>
}

export const App = createContext<AppContext | null>(null)

export function useApp(): AppContext {
  const ctx = useContext(App)
  if (!ctx) throw new Error('useApp outside the app')
  return ctx
}
