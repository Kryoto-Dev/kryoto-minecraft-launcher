import { useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { isTauri } from '@/lib/bridge'
import { api } from '@/lib/api'

export { isTauri }

/**
 * The frameless window's controls (Kryoto Desktop's lib/window.ts, without
 * the Store's web view, which this app has none of).
 */

export type WindowState = { maximized: boolean; fullscreen: boolean }

export function useWindowState(): WindowState {
  const [state, setState] = useState<WindowState>({ maximized: false, fullscreen: false })
  useEffect(() => {
    if (!isTauri()) return
    const win = getCurrentWindow()
    let stop: (() => void) | undefined
    let cancelled = false
    const read = () => void Promise.all([win.isMaximized(), win.isFullscreen()]).then(([maximized, fullscreen]) => !cancelled && setState({ maximized, fullscreen }))
    read()
    void win.onResized(read).then((fn) => (cancelled ? fn() : (stop = fn)))
    return () => {
      cancelled = true
      stop?.()
    }
  }, [])
  useEffect(() => {
    document.documentElement.dataset.frame = state.maximized || state.fullscreen ? 'square' : 'round'
  }, [state])
  return state
}

export const minimizeWindow = () => (isTauri() ? getCurrentWindow().minimize() : Promise.resolve())
export const toggleMaximize = () => (isTauri() ? getCurrentWindow().toggleMaximize() : Promise.resolve())
export const closeWindow = () => (isTauri() ? getCurrentWindow().close() : Promise.resolve())
export const showWindow = () => (isTauri() ? getCurrentWindow().show().then(() => getCurrentWindow().setFocus()) : Promise.resolve())

export function openExternal(url: string) {
  if (!isTauri()) {
    window.open(url, '_blank', 'noopener')
    return Promise.resolve()
  }
  return api.openExternal(url)
}

export const isWindowsHost = () => typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent)
