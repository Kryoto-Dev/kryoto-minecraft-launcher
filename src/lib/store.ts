import { useSyncExternalStore } from 'react'
import { on } from '@/lib/bridge'
import { api, type GameState, type Instance, type LogLine, type Profiles, type Task } from '@/lib/api'
import { logError } from '@/lib/log'

/**
 * What the whole window shares, kept current from the native side's events:
 * `tasks` (every progress change), `game-state` and `game-log` (a game
 * starting, its output, it stopping) and `instances-changed`. One copy, so
 * the sidebar, the instance page and the bottom bar never disagree.
 */

function store<T>(initial: T) {
  let value = initial
  const listeners = new Set<() => void>()
  return {
    get: () => value,
    set(next: T) {
      value = next
      listeners.forEach((l) => l())
    },
    subscribe(l: () => void) {
      listeners.add(l)
      return () => void listeners.delete(l)
    },
  }
}

const instances = store<Instance[] | null>(null)
const tasks = store<Task[]>([])
const profiles = store<Profiles>({ profiles: [], active: null })
const games = store<Record<string, GameState>>({})
const logs = new Map<string, ReturnType<typeof store<LogLine[]>>>()

function logStore(id: string) {
  let s = logs.get(id)
  if (!s) {
    s = store<LogLine[]>([])
    logs.set(id, s)
  }
  return s
}

const MAX_LOG = 5000

export const live = {
  async refreshInstances() {
    try {
      instances.set(await api.instances())
    } catch (e) {
      logError('instances', e)
      if (instances.get() === null) instances.set([])
    }
  },
  async refreshProfiles() {
    try {
      profiles.set(await api.profiles())
    } catch (e) {
      logError('profiles', e)
    }
  },
  async loadLog(id: string) {
    try {
      logStore(id).set(await api.gameLog(id))
    } catch (e) {
      logError('log', e)
    }
  },
  instance: (id: string) => instances.get()?.find((i) => i.id === id) ?? null,
  activeProfile: () => activeProfile(profiles.get()),
}

let started = false

/** Start listening (once, from App). */
export function startLive() {
  if (started) return
  started = true
  void live.refreshInstances()
  void live.refreshProfiles()
  void api.tasks().then(tasks.set).catch(() => {})
  void api
    .gameStates()
    .then((list) => games.set(Object.fromEntries(list.map((g) => [g.instance, g]))))
    .catch(() => {})

  let finished = new Set<string>()
  void on<Task[]>('tasks', (list) => {
    tasks.set(list)
    // A job that just ended may have installed something.
    const now = new Set(list.filter((t) => t.state !== 'running').map((t) => t.id))
    if ([...now].some((id) => !finished.has(id))) void live.refreshInstances()
    finished = now
  })
  void on<GameState>('game-state', (state) => {
    games.set({ ...games.get(), [state.instance]: state })
    if (state.running) logStore(state.instance).set(logStore(state.instance).get())
    void live.refreshInstances()
  })
  void on<{ instance: string; lines: LogLine[] }>('game-log', ({ instance, lines }) => {
    const s = logStore(instance)
    const next = s.get().concat(lines)
    s.set(next.length > MAX_LOG ? next.slice(next.length - MAX_LOG) : next)
  })
  void on('instances-changed', () => void live.refreshInstances())
}

export const useInstances = () => useSyncExternalStore(instances.subscribe, instances.get)
export const useTasks = () => useSyncExternalStore(tasks.subscribe, tasks.get)
export const useProfiles = () => useSyncExternalStore(profiles.subscribe, profiles.get)
export const useGames = () => useSyncExternalStore(games.subscribe, games.get)
export function useLog(id: string) {
  const s = logStore(id)
  return useSyncExternalStore(s.subscribe, s.get)
}

/** The running job for an instance, if any. */
export function taskFor(list: Task[], instance: string) {
  return list.find((t) => t.instance === instance && t.state === 'running') ?? null
}

/** Progress of a task as 0..1, or null while it has no size yet. */
export function taskFraction(t: Task): number | null {
  if (t.totalBytes > 0) return Math.min(1, t.doneBytes / t.totalBytes)
  if (t.totalFiles > 0) return Math.min(1, t.doneFiles / t.totalFiles)
  return null
}

export function activeProfile(p: Profiles) {
  return p.profiles.find((x) => x.id === p.active) ?? null
}
