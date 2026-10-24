import type { AppInfo, ContentItem, GameState, Instance, LogLine, Profiles, Settings, Task, VersionList } from '@/lib/api'

/**
 * `pnpm dev` in a plain browser: the native commands answered with sample
 * data, so every screen can be opened and clicked through without building
 * the app (as Kryoto Desktop's preview does). Installs and games are acted
 * out with timers; nothing is downloaded.
 */

type Handler = (payload: unknown) => void
const handlers = new Map<string, Set<Handler>>()

export const previewBus = {
  on(event: string, handler: Handler) {
    let set = handlers.get(event)
    if (!set) handlers.set(event, (set = new Set()))
    set.add(handler)
    return () => void set!.delete(handler)
  },
  emit(event: string, payload: unknown) {
    handlers.get(event)?.forEach((h) => h(payload))
  },
}

const now = () => Math.floor(Date.now() / 1000)
const day = 86400

const info: AppInfo = { version: __APP_VERSION__, dataDir: 'C:\\Users\\you\\AppData\\Roaming\\to.kryo.minecraft', platform: 'windows-x86_64', totalMemoryMb: 16384 }

let settings: Settings = {
  javaPath: null,
  memoryMb: 4096,
  jvmArgs: '',
  width: 854,
  height: 480,
  fullscreen: false,
  afterLaunch: 'keep',
  parallelDownloads: 16,
  palette: 'oled',
  radius: 'pill',
  font: 'teletext',
  chick: true,
  firstRunDone: new URLSearchParams(location.search).get('first') !== '1',
}

const overrides = { javaPath: null, memoryMb: null, jvmArgs: null, width: null, height: null, fullscreen: null }
let instances: Instance[] = new URLSearchParams(location.search).get('empty') === '1' ? [] : [
  { id: 'survival', name: 'Survival', gameVersion: '26.3', loader: { kind: 'vanilla', version: null }, launchVersion: '26.3', versionType: 'release', installed: true, created: now() - 40 * day, lastPlayed: now() - 3600, playSeconds: 152_000, icon: 'grass', overrides },
  { id: 'sodium-fabric', name: 'Fabric 1.21.1', gameVersion: '1.21.1', loader: { kind: 'fabric', version: '0.20.1' }, launchVersion: 'fabric-loader-0.20.1-1.21.1', versionType: 'release', installed: true, created: now() - 20 * day, lastPlayed: now() - 2 * day, playSeconds: 30_000, icon: 'diamond', overrides },
  { id: 'all-the-mods', name: 'Forge 1.20.1', gameVersion: '1.20.1', loader: { kind: 'forge', version: '1.20.1-47.4.0' }, launchVersion: null, versionType: 'release', installed: false, created: now() - 2 * day, lastPlayed: null, playSeconds: 0, icon: 'redstone', overrides },
  { id: 'beta', name: 'Beta 1.7.3', gameVersion: 'b1.7.3', loader: { kind: 'vanilla', version: null }, launchVersion: 'b1.7.3', versionType: 'old_beta', installed: true, created: now() - 9 * day, lastPlayed: now() - 9 * day, playSeconds: 4_000, icon: 'sand', overrides },
]

let profiles: Profiles = { profiles: [{ id: 'b50ad385829d3141a2167e7d7539ba7f', name: 'Steve', added: now() }], active: 'b50ad385829d3141a2167e7d7539ba7f' }
if (!settings.firstRunDone) profiles = { profiles: [], active: null }

const versions: VersionList = (() => {
  const v: VersionList['versions'] = []
  const at = (y: number, m: number) => new Date(Date.UTC(y, m, 10)).toISOString()
  for (let s = 5; s >= 1; s--) v.push({ id: `26.4-snapshot-${s}`, kind: 'snapshot', releaseTime: at(2026, 9), installed: false })
  for (const [id, y, m] of [['26.3', 2026, 8], ['26.2', 2026, 5], ['26.1', 2026, 2], ['1.21.11', 2025, 11], ['1.21.1', 2024, 7], ['1.20.1', 2023, 5], ['1.18.2', 2022, 2], ['1.16.5', 2021, 0], ['1.12.2', 2017, 8], ['1.8.9', 2015, 11], ['1.7.10', 2014, 5], ['1.5.2', 2013, 4]] as const)
    v.push({ id, kind: 'release', releaseTime: at(y, m), installed: id === '26.3' || id === '1.21.1' })
  for (const id of ['b1.8.1', 'b1.7.3', 'b1.6.6']) v.push({ id, kind: 'old_beta', releaseTime: at(2011, 6), installed: id === 'b1.7.3' })
  for (const id of ['a1.2.6', 'a1.1.2_01', 'c0.30_01c', 'rd-132211']) v.push({ id, kind: 'old_alpha', releaseTime: at(2010, 9), installed: false })
  return { latestRelease: '26.3', latestSnapshot: '26.4-snapshot-5', versions: v }
})()

let tasks: Task[] = []
const games: Record<string, GameState> = {}
const logs: Record<string, LogLine[]> = {}

const content: Record<string, ContentItem[]> = {
  'sodium-fabric:mod': [
    { file: 'sodium-fabric-0.8.13+mc1.21.1.jar', kind: 'mod', name: 'Sodium', version: '0.8.13+mc1.21.1', description: 'The fastest and most compatible rendering optimization mod for Minecraft.', authors: 'JellySquid, IMS', enabled: true, size: 1_300_000, modified: now(), icon: null, source: null },
    { file: 'lithium-fabric-0.15.jar', kind: 'mod', name: 'Lithium', version: '0.15.0', description: 'No-compromises game logic optimization.', authors: 'jellysquid3', enabled: true, size: 700_000, modified: now(), icon: null, source: null },
    { file: 'journeymap-1.21.1.jar.disabled', kind: 'mod', name: 'JourneyMap', version: '6.0.0', description: 'Real-time map in game or in a web browser as you explore.', authors: 'techbrew', enabled: false, size: 5_000_000, modified: now(), icon: null, source: null },
  ],
  'survival:world': [{ file: 'New World', kind: 'world', name: 'New World', version: null, description: null, authors: null, enabled: true, size: 0, modified: now() - 3600, icon: null, source: null }],
}

function emitTasks() {
  previewBus.emit('tasks', tasks)
}

function fakeTask(title: string, instance: string | null, total: number, done: () => void) {
  const id = `t${Date.now()}`
  const t: Task = { id, title, instance, stage: 'Libraries', state: 'running', doneBytes: 0, totalBytes: total, doneFiles: 0, totalFiles: 420, speed: 18_000_000, downloaded: 0, error: null, started: now(), finished: null, retry: null }
  tasks = [...tasks, t]
  emitTasks()
  const timer = window.setInterval(() => {
    const cur = tasks.find((x) => x.id === id)
    if (!cur || cur.state !== 'running') return window.clearInterval(timer)
    const doneBytes = Math.min(total, cur.doneBytes + total / 25)
    const stage = doneBytes > total * 0.7 ? 'Java 21' : doneBytes > total * 0.3 ? 'Assets' : 'Libraries'
    tasks = tasks.map((x) => (x.id === id ? { ...x, doneBytes, downloaded: doneBytes, doneFiles: Math.round((doneBytes / total) * 420), stage } : x))
    if (doneBytes >= total) {
      window.clearInterval(timer)
      tasks = tasks.map((x) => (x.id === id ? { ...x, state: 'done', stage: 'Done', finished: now(), speed: 0 } : x))
      done()
    }
    emitTasks()
  }, 200)
  return id
}

function startGame(id: string) {
  games[id] = { instance: id, running: true, started: now(), pid: 1234, exit: null }
  logs[id] = [{ line: `Kryoto Minecraft Launcher ${info.version} - starting`, stream: 'launcher' }]
  previewBus.emit('game-state', games[id])
  const lines: LogLine[] = [
    { line: '[12:00:01] [main/INFO]: Setting user: Steve', stream: 'out' },
    { line: '[12:00:02] [Render thread/INFO]: Backend library: LWJGL version 3.4.3', stream: 'out' },
    { line: '[12:00:03] [Download-1/ERROR]: Couldn\'t get player attributes (offline profile)', stream: 'err' },
    { line: '[12:00:04] [Render thread/WARN]: Missing sound for event: minecraft:item.goat_horn.play', stream: 'warn' },
    { line: '[12:00:05] [Render thread/INFO]: Sound engine started', stream: 'out' },
  ]
  lines.forEach((l, i) =>
    window.setTimeout(() => {
      logs[id]!.push(l)
      previewBus.emit('game-log', { instance: id, lines: [l] })
    }, 300 * (i + 1)),
  )
}

export function startPreview() {}

export async function previewCall<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  await new Promise((r) => setTimeout(r, 60))
  const id = args.id as string
  const r = (v: unknown) => v as T
  switch (command) {
    case 'app_info':
      return r(info)
    case 'settings_get':
      return r(settings)
    case 'settings_set':
      settings = args.settings as Settings
      return r(settings)
    case 'versions_list':
      return r(versions)
    case 'version_details':
      return r({ id, kind: 'release', releaseTime: null, javaMajor: id.startsWith('26') ? 25 : id.startsWith('1.2') ? 21 : 8, mainClass: null, libraryCount: 64, clientSize: 28_000_000, assets: '34', assetsSize: 640_000_000 })
    case 'loader_versions': {
      const game = args.game as string
      if (/^(b|a|c|rd)/.test(game) || game.includes('snapshot')) return r([])
      const kind = args.kind as string
      if (kind === 'neoforge' && !/^1\.2[01]|^26/.test(game)) return r([])
      const base = kind === 'forge' ? '47.4.' : kind === 'neoforge' ? '21.1.' : '0.20.'
      return r([0, 1, 2, 3].map((n) => ({ version: `${base}${3 - n}`, label: `${base}${3 - n}`, stable: true, recommended: n === 0 })))
    }
    case 'instances_list':
      return r(instances)
    case 'instance_create': {
      const n = args.instance as Pick<Instance, 'name' | 'gameVersion' | 'versionType' | 'loader' | 'icon'>
      const created: Instance = { ...n, id: `i${Date.now()}`, launchVersion: null, installed: false, created: now(), lastPlayed: null, playSeconds: 0, overrides }
      instances = [...instances, created]
      fakeTask(`Installing ${created.name}`, created.id, 600_000_000, () => {
        instances = instances.map((i) => (i.id === created.id ? { ...i, installed: true } : i))
        previewBus.emit('instances-changed', null)
      })
      return r(created)
    }
    case 'instance_update': {
      const patch = args.patch as Partial<Instance>
      instances = instances.map((i) => (i.id === id ? { ...i, ...Object.fromEntries(Object.entries(patch).filter(([, v]) => v !== undefined)) } : i))
      return r(instances.find((i) => i.id === id))
    }
    case 'instance_delete':
      instances = instances.filter((i) => i.id !== id)
      return r(null)
    case 'instance_install':
      fakeTask(args.repair ? 'Repairing' : 'Installing', id, 200_000_000, () => {
        instances = instances.map((i) => (i.id === id ? { ...i, installed: true } : i))
        previewBus.emit('instances-changed', null)
      })
      return r(instances.find((i) => i.id === id))
    case 'launch': {
      const inst = instances.find((i) => i.id === id)
      if (!profiles.active) throw 'Make a profile first: pick the name you play under.'
      if (inst && !inst.installed) {
        fakeTask(`Starting ${inst.name}`, id, 300_000_000, () => {
          instances = instances.map((i) => (i.id === id ? { ...i, installed: true } : i))
          previewBus.emit('instances-changed', null)
          startGame(id)
        })
      } else startGame(id)
      return r(1234)
    }
    case 'game_stop':
      games[id] = { instance: id, running: false, started: null, pid: null, exit: { code: 0, seconds: 60, hint: null, crashReport: null, stoppedByPlayer: true } }
      previewBus.emit('game-state', games[id])
      return r(true)
    case 'game_states':
      return r(Object.values(games))
    case 'game_log':
      return r(logs[id] ?? [])
    case 'tasks_list':
      return r(tasks)
    case 'task_cancel':
      tasks = tasks.map((t) => (t.id === id ? { ...t, state: 'cancelled', stage: 'Cancelled', finished: now() } : t))
      emitTasks()
      return r(null)
    case 'tasks_clear':
      tasks = tasks.filter((t) => t.state === 'running')
      emitTasks()
      return r(null)
    case 'profiles_list':
      return r(profiles)
    case 'profile_add': {
      const name = args.name as string
      if (!/^[A-Za-z0-9_]{3,16}$/.test(name)) throw 'A name is 3 to 16 letters, numbers or _.'
      const p = { id: `p-${name}`, name, added: now() }
      profiles = { profiles: [...profiles.profiles, p], active: p.id }
      return r(p)
    }
    case 'profile_set_active':
      profiles = { ...profiles, active: id }
      return r(null)
    case 'profile_remove':
      profiles = { profiles: profiles.profiles.filter((p) => p.id !== id), active: profiles.active === id ? null : profiles.active }
      return r(null)
    case 'content_list':
      return r(content[`${id}:${args.kind as string}`] ?? [])
    case 'content_set_enabled': {
      const list = content[`${id}:${args.kind as string}`] ?? []
      const file = args.file as string
      content[`${id}:${args.kind as string}`] = list.map((c) => (c.file === file ? { ...c, enabled: !!args.enabled, file: args.enabled ? file.replace(/\.disabled$/, '') : `${file}.disabled` } : c))
      return r(file)
    }
    case 'content_delete':
      content[`${id}:${args.kind as string}`] = (content[`${id}:${args.kind as string}`] ?? []).filter((c) => c.file !== args.file)
      return r(null)
    case 'content_pick':
    case 'content_import':
      return r([])
    case 'modrinth_search':
      return r({
        hits: ['Sodium', 'Iris Shaders', 'Lithium', 'Mod Menu', 'Fabric API', 'Xaero\'s Minimap'].map((title, i) => ({
          project_id: `p${i}`,
          slug: title.toLowerCase().replace(/\W+/g, '-'),
          title,
          description: 'A sample result in the browser preview. In the app these come from Modrinth.',
          icon_url: null,
          downloads: 90_000_000 / (i + 1),
          author: 'someone',
          categories: [],
          date_modified: '',
        })),
        offset: 0,
        limit: 20,
        total_hits: 6,
      })
    case 'modrinth_install':
      await new Promise((r2) => setTimeout(r2, 900))
      return r([args.title])
    case 'java_probe':
      return r({ path: args.path, version: '21.0.7', major: 21, arch: 'amd64', vendor: 'Microsoft' })
    case 'java_pick':
      return r('C:\\Program Files\\Java\\jdk-21\\bin\\java.exe')
    case 'open_external':
    case 'open_data_folder':
    case 'instance_open_folder':
    case 'open_crash_report':
    case 'log_write':
      return r(null)
    default:
      throw `The preview has no answer for ${command}.`
  }
}
