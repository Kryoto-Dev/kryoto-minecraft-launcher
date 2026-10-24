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

/**
 * Resizing from the window's edges on Linux, where a frameless GTK window has
 * no resize border of its own (Windows draws one). A press within a few
 * pixels of an edge hands the resize to the window manager.
 */
const EDGE = 5
type Direction = 'North' | 'South' | 'East' | 'West' | 'NorthEast' | 'NorthWest' | 'SouthEast' | 'SouthWest'

function edgeAt(x: number, y: number): Direction | null {
  const w = window.innerWidth
  const h = window.innerHeight
  const n = y < EDGE, s = y > h - EDGE, west = x < EDGE, e = x > w - EDGE
  if (n && west) return 'NorthWest'
  if (n && e) return 'NorthEast'
  if (s && west) return 'SouthWest'
  if (s && e) return 'SouthEast'
  if (n) return 'North'
  if (s) return 'South'
  if (west) return 'West'
  if (e) return 'East'
  return null
}

const CURSORS: Record<Direction, string> = {
  North: 'ns-resize', South: 'ns-resize', East: 'ew-resize', West: 'ew-resize',
  NorthEast: 'nesw-resize', SouthWest: 'nesw-resize', NorthWest: 'nwse-resize', SouthEast: 'nwse-resize',
}

export function installEdgeResize() {
  if (!isTauri() || !/Linux/i.test(navigator.userAgent)) return
  const root = document.documentElement
  window.addEventListener('mousemove', (e) => {
    const edge = document.documentElement.dataset.frame === 'square' ? null : edgeAt(e.clientX, e.clientY)
    root.style.cursor = edge ? CURSORS[edge] : ''
  })
  window.addEventListener(
    'mousedown',
    (e) => {
      if (e.button !== 0 || root.dataset.frame === 'square') return
      const edge = edgeAt(e.clientX, e.clientY)
      if (!edge) return
      e.preventDefault()
      e.stopPropagation()
      void getCurrentWindow().startResizeDragging(edge)
    },
    true,
  )
}
