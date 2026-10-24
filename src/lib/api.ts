import { call } from '@/lib/bridge'

/**
 * Every native command the UI uses, typed. The shapes are the Rust structs in
 * crates/kryoto-mc (serde camelCase) and src-tauri/src/lib.rs.
 */

export type AppInfo = {
  version: string
  dataDir: string
  microsoftAvailable: boolean
  platform: string
  totalMemoryMb: number
}

export type Settings = {
  javaPath: string | null
  memoryMb: number
  jvmArgs: string
  width: number
  height: number
  fullscreen: boolean
  afterLaunch: 'keep' | 'minimize'
  parallelDownloads: number
  palette: string
  radius: string
  font: string
  firstRunDone: boolean
}

export type VersionKind = 'release' | 'snapshot' | 'old_beta' | 'old_alpha'

export type VersionRow = { id: string; kind: VersionKind; releaseTime: string; installed: boolean }
export type VersionList = { latestRelease: string; latestSnapshot: string; versions: VersionRow[] }

export type VersionDetails = {
  id: string
  kind: string
  releaseTime: string | null
  javaMajor: number
  mainClass: string | null
  libraryCount: number
  clientSize: number | null
  assets: string | null
  assetsSize: number | null
}

export type LoaderKind = 'vanilla' | 'fabric' | 'quilt' | 'forge' | 'neoforge'
export type LoaderVersion = { version: string; label: string; stable: boolean; recommended: boolean }
export type Loader = { kind: LoaderKind; version: string | null }

export type Overrides = {
  javaPath: string | null
  memoryMb: number | null
  jvmArgs: string | null
  width: number | null
  height: number | null
  fullscreen: boolean | null
}

export type Instance = {
  id: string
  name: string
  gameVersion: string
  loader: Loader
  launchVersion: string | null
  versionType: string
  installed: boolean
  created: number
  lastPlayed: number | null
  playSeconds: number
  icon: string | null
  overrides: Overrides
}

export type TaskState = 'running' | 'done' | 'failed' | 'cancelled'
export type Task = {
  id: string
  title: string
  instance: string | null
  stage: string
  state: TaskState
  doneBytes: number
  totalBytes: number
  doneFiles: number
  totalFiles: number
  speed: number
  downloaded: number
  error: string | null
  started: number
  finished: number | null
  retry: { command: string; args: Record<string, unknown> } | null
}

export type Account = {
  id: string
  kind: 'microsoft' | 'offline'
  name: string
  xuid: string | null
  skinUrl: string | null
  added: number
}
export type Accounts = { accounts: Account[]; active: string | null }
export type DeviceCode = { userCode: string; verificationUri: string; expiresIn: number }

export type LogLine = { line: string; stream: 'out' | 'warn' | 'err' | 'launcher' }
export type Exit = { code: number | null; seconds: number; hint: string | null; crashReport: string | null; stoppedByPlayer: boolean }
export type GameState = { instance: string; running: boolean; started: number | null; pid: number | null; exit: Exit | null }

export type ContentKind = 'mod' | 'resourcepack' | 'shaderpack' | 'world'
export type ContentSource = { projectId: string; versionId: string; title: string; iconUrl: string | null; versionNumber: string | null }
export type ContentItem = {
  file: string
  kind: ContentKind
  name: string
  version: string | null
  description: string | null
  authors: string | null
  enabled: boolean
  size: number
  modified: number
  icon: string | null
  source: ContentSource | null
}

export type ModrinthHit = {
  project_id: string
  slug: string
  title: string
  description: string
  icon_url: string | null
  downloads: number
  author: string
  categories: string[]
  date_modified: string
}
export type ModrinthResult = { hits: ModrinthHit[]; offset: number; limit: number; total_hits: number }

export type JavaInfo = { path: string; version: string; major: number; arch: string; vendor: string }

export const api = {
  appInfo: () => call<AppInfo>('app_info'),
  settings: () => call<Settings>('settings_get'),
  saveSettings: (settings: Settings) => call<Settings>('settings_set', { settings }),
  openExternal: (url: string) => call<void>('open_external', { url }),
  openDataFolder: (sub?: 'logs') => call<void>('open_data_folder', { sub: sub ?? null }),

  versions: (refresh = false) => call<VersionList>('versions_list', { refresh }),
  versionDetails: (id: string) => call<VersionDetails>('version_details', { id }),
  loaderVersions: (kind: LoaderKind, game: string) => call<LoaderVersion[]>('loader_versions', { kind, game }),

  instances: () => call<Instance[]>('instances_list'),
  createInstance: (instance: { name: string; gameVersion: string; versionType: string; loader: Loader; icon: string | null }) =>
    call<Instance>('instance_create', { instance }),
  updateInstance: (id: string, patch: { name?: string; icon?: string; overrides?: Overrides; loader?: Loader }) =>
    call<Instance>('instance_update', { id, patch }),
  deleteInstance: (id: string) => call<void>('instance_delete', { id }),
  install: (id: string, repair = false) => call<Instance>('instance_install', { id, repair }),
  openFolder: (id: string, sub?: string) => call<void>('instance_open_folder', { id, sub: sub ?? null }),
  openCrashReport: (id: string, path: string) => call<void>('open_crash_report', { id, path }),

  launch: (id: string, server?: string | null, world?: string | null) => call<number>('launch', { id, server: server ?? null, world: world ?? null }),
  stop: (id: string) => call<boolean>('game_stop', { id }),
  gameStates: () => call<GameState[]>('game_states'),
  gameLog: (id: string) => call<LogLine[]>('game_log', { id }),

  tasks: () => call<Task[]>('tasks_list'),
  cancelTask: (id: string) => call<void>('task_cancel', { id }),
  clearTasks: () => call<void>('tasks_clear'),

  accounts: () => call<Accounts>('accounts_list'),
  addOffline: (name: string) => call<Account>('account_offline_add', { name }),
  setActiveAccount: (id: string) => call<void>('account_set_active', { id }),
  removeAccount: (id: string) => call<void>('account_remove', { id }),
  microsoftStart: () => call<DeviceCode>('account_microsoft_start'),
  microsoftFinish: () => call<Account>('account_microsoft_finish'),
  microsoftCancel: () => call<void>('account_microsoft_cancel'),

  content: (id: string, kind: ContentKind) => call<ContentItem[]>('content_list', { id, kind }),
  setContentEnabled: (id: string, kind: ContentKind, file: string, enabled: boolean) => call<string>('content_set_enabled', { id, kind, file, enabled }),
  deleteContent: (id: string, kind: ContentKind, file: string) => call<void>('content_delete', { id, kind, file }),
  importContent: (id: string, files: string[], kind?: ContentKind) =>
    call<{ file: string; kind: ContentKind }[]>('content_import', { id, files, kind: kind ?? null }),
  pickContent: (id: string, kind: ContentKind) => call<{ file: string; kind: ContentKind }[]>('content_pick', { id, kind }),
  modrinthSearch: (id: string, kind: ContentKind, query: string, sort: string, offset: number) =>
    call<ModrinthResult>('modrinth_search', { id, kind, query, sort, offset }),
  modrinthInstall: (id: string, kind: ContentKind, project: string, title: string) => call<string[]>('modrinth_install', { id, kind, project, title }),

  probeJava: (path: string) => call<JavaInfo>('java_probe', { path }),
  pickJava: () => call<string | null>('java_pick'),
}

/** Run what a task's "Try again" stands for. */
export function retryTask(task: Task) {
  if (!task.retry) return Promise.resolve()
  return call(task.retry.command, task.retry.args)
}

export const LOADERS: { kind: LoaderKind; label: string }[] = [
  { kind: 'vanilla', label: 'Vanilla' },
  { kind: 'fabric', label: 'Fabric' },
  { kind: 'quilt', label: 'Quilt' },
  { kind: 'forge', label: 'Forge' },
  { kind: 'neoforge', label: 'NeoForge' },
]

export const loaderLabel = (kind: LoaderKind) => LOADERS.find((l) => l.kind === kind)?.label ?? kind

/** "Fabric 1.21.1", or just "1.21.1". */
export function instanceLabel(i: Pick<Instance, 'loader' | 'gameVersion'>) {
  return i.loader.kind === 'vanilla' ? i.gameVersion : `${loaderLabel(i.loader.kind)} ${i.gameVersion}`
}

export const KIND_LABEL: Record<VersionKind, string> = {
  release: 'Release',
  snapshot: 'Snapshot',
  old_beta: 'Beta',
  old_alpha: 'Alpha',
}
