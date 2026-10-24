import { useSyncExternalStore } from 'react'
import { getVersion } from '@tauri-apps/api/app'
import { relaunch } from '@tauri-apps/plugin-process'
import { check as tauriCheck, type Update } from '@tauri-apps/plugin-updater'
import { errorText, isTauri } from '@/lib/bridge'

/**
 * The in-app updater, as Kryoto Desktop and Forge do it (Desktop's
 * src/lib/updates.ts, with this app's download page in its messages): one
 * store outside the component tree, so the prompt on launch and "Check for
 * updates" in Settings never download the same installer twice.
 *
 * Where it looks: `latest.json` on the newest GitHub release
 * (`plugins.updater` in tauri.conf.json), written by the release workflow and
 * signed with the key whose public half is in that file.
 */

export const DOWNLOAD_PAGE = 'https://github.com/Kryoto-Dev/kryoto-minecraft-launcher/releases/latest'

export type UpdateStatus = 'idle' | 'checking' | 'current' | 'available' | 'downloading' | 'ready' | 'failed'

export type UpdateState = {
  status: UpdateStatus
  current: string
  update: Update | null
  error: string | null
  checkedAt: number | null
  got: number
  total: number | null
}

const CHECK_TIMEOUT_MS = 20_000

let state: UpdateState = { status: 'idle', current: '', update: null, error: null, checkedAt: null, got: 0, total: null }
const listeners = new Set<() => void>()

function set(patch: Partial<UpdateState>) {
  state = { ...state, ...patch }
  for (const l of listeners) l()
}

const busy = () => state.status === 'checking' || state.status === 'downloading' || state.status === 'ready'

let launchChecked = false

export const updates = {
  snapshot: () => state,

  async loadVersion() {
    if (state.current || !isTauri()) return
    try {
      set({ current: await getVersion() })
    } catch {
      // Only a label.
    }
  },

  async check() {
    if (!isTauri()) {
      set({ status: 'failed', error: 'The browser preview has no updater. Run the app to check for updates.', checkedAt: Date.now() })
      return
    }
    if (busy()) return
    set({ status: 'checking', error: null })
    try {
      const found = await tauriCheck({ timeout: CHECK_TIMEOUT_MS })
      set({ update: found, status: found ? 'available' : 'current', error: null, checkedAt: Date.now(), got: 0, total: null })
    } catch (e) {
      set({ status: 'failed', update: null, error: errorText(e), checkedAt: Date.now() })
    }
  },

  /** Once per run, as soon as there is a connection. */
  async checkOnLaunch() {
    if (launchChecked || !isTauri()) return
    if (!navigator.onLine) {
      window.addEventListener('online', () => void updates.checkOnLaunch(), { once: true })
      return
    }
    launchChecked = true
    await updates.check()
  },

  async install() {
    const found = state.update
    if (!found || state.status === 'downloading' || state.status === 'ready') return
    set({ status: 'downloading', error: null, got: 0, total: null })
    try {
      await found.downloadAndInstall((event) => {
        if (event.event === 'Started') set({ total: event.data.contentLength ?? null, got: 0 })
        else if (event.event === 'Progress') set({ got: state.got + event.data.chunkLength })
        else if (event.event === 'Finished') set({ status: 'ready' })
      })
      await relaunch()
    } catch (e) {
      set({ status: 'failed', error: errorText(e) })
    }
  },
}

function subscribe(cb: () => void) {
  listeners.add(cb)
  return () => {
    listeners.delete(cb)
  }
}

export function useUpdates(): UpdateState {
  return useSyncExternalStore(subscribe, updates.snapshot, updates.snapshot)
}

export function updateFraction(s: UpdateState): number | null {
  if (!s.total || s.total <= 0) return null
  return Math.min(1, s.got / s.total)
}

export function explainUpdateError(error: string | null): { headline: string; retryable: boolean } | null {
  if (!error) return null
  const e = error.toLowerCase()
  if (e.includes('different key') || e.includes('signature verification failed') || e.includes('unexpected signature algorithm') || e.includes('invalid encoding in minisign')) {
    return { headline: 'This release was signed with a key this app does not trust, so it was refused. Download the new version from the releases page instead.', retryable: false }
  }
  if (e.includes('was not found in the response') || e.includes('platforms` object')) {
    return { headline: 'The new version has no build for this system yet. Nothing is wrong here.', retryable: false }
  }
  if (e.includes('could not fetch a valid release json')) {
    return { headline: 'The update server answered with something that is not a release. Try again in a few minutes.', retryable: true }
  }
  if (e.includes('timed out') || e.includes('timeout') || e.includes('dns') || e.includes('connect') || e.includes('network')) {
    return { headline: 'The update server could not be reached.', retryable: true }
  }
  return null
}
