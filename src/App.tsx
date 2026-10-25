import { useCallback, useEffect, useMemo, useState } from 'react'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { Plus } from 'lucide-react'
import { Busy, Button } from '@/ui'
import { Toasts, toast, toastError } from '@/ui/Toasts'
import { api, type AppInfo, type GameState, type Instance, type Settings, type VersionList } from '@/lib/api'
import { App as AppCtx, useApp } from '@/lib/app-context'
import { errorText, isTauri, on } from '@/lib/bridge'
import { applyLook } from '@/lib/look'
import { isModpack } from '@/lib/modpack'
import { live, startLive, useInstances, useTasks } from '@/lib/store'
import { minimizeWindow, showWindow } from '@/lib/window'
import { ProfileDialog, ProfileMenu } from '@/profiles/Profiles'
import { setProfilePrompt } from '@/profiles/require'
import { Welcome } from '@/boot/Welcome'
import { Mobs } from '@/mobs/Mobs'
import { DownloadsPage } from '@/downloads/DownloadsPage'
import { Home } from '@/library/Home'
import { InstancePage } from '@/library/InstancePage'
import { JoinDialog } from '@/library/JoinDialog'
import { Sidebar } from '@/library/Sidebar'
import { SettingsPage } from '@/settings/SettingsPage'
import { BottomBar } from '@/shell/BottomBar'
import { NavBar, type Tab } from '@/shell/NavBar'
import { TitleBar } from '@/shell/TitleBar'
import { UpdatePrompt } from '@/shell/UpdatePrompt'
import { NewInstanceDialog } from '@/versions/NewInstanceDialog'
import { VersionsPage } from '@/versions/VersionsPage'

export default function App() {
  const [info, setInfo] = useState<AppInfo | null>(null)
  const [settings, setSettings] = useState<Settings | null>(null)
  const [bootError, setBootError] = useState<string | null>(null)

  useEffect(() => {
    startLive()
    Promise.all([api.appInfo(), api.settings()])
      .then(([i, s]) => {
        setInfo(i)
        setSettings(s)
        applyLook(s)
      })
      .catch((e) => setBootError(errorText(e)))
      .finally(() => requestAnimationFrame(() => void showWindow()))
  }, [])

  const saveSettings = useCallback(
    async (patch: Partial<Settings>) => {
      if (!settings) return
      const next = { ...settings, ...patch }
      setSettings(next)
      applyLook(next)
      try {
        setSettings(await api.saveSettings(next))
      } catch (e) {
        toastError(e)
      }
    },
    [settings],
  )
  const ctx = useMemo(() => (info && settings ? { info, settings, saveSettings } : null), [info, settings, saveSettings])

  if (bootError) {
    return (
      <div className="grid h-full place-items-center p-8 text-center">
        <p className="max-w-md text-xs text-destructive">The launcher couldn't start: {bootError}</p>
      </div>
    )
  }
  if (!ctx) {
    return (
      <div className="grid h-full place-items-center">
        <Busy className="size-6" />
      </div>
    )
  }
  return (
    <AppCtx.Provider value={ctx}>
      <Shell />
      <Toasts />
    </AppCtx.Provider>
  )
}

function Shell() {
  const [tab, setTab] = useState<Tab>('play')
  const [selected, setSelected] = useState<string | null>(null)
  const [addingProfile, setAddingProfile] = useState(false)
  // A game asked to start with no profile yet: what to run once there is one.
  const [firstProfile, setFirstProfile] = useState<{ then: () => void } | null>(null)
  useEffect(() => {
    setProfilePrompt((then) => setFirstProfile({ then }))
    return () => setProfilePrompt(null)
  }, [])
  const [creating, setCreating] = useState<{ initial?: string } | null>(null)
  const [joining, setJoining] = useState<Instance | null>(null)
  const [versions, setVersions] = useState<VersionList | null>(null)
  const [versionsError, setVersionsError] = useState<string | null>(null)
  const instances = useInstances()
  const tasks = useTasks()
  const { settings, saveSettings } = useApp()

  const loadVersions = useCallback((refresh: boolean) => {
    setVersionsError(null)
    api
      .versions(refresh)
      .then(setVersions)
      .catch((e) => setVersionsError(errorText(e)))
  }, [])
  useEffect(() => loadVersions(false), [loadVersions])

  // Minimize while playing, if asked to.
  useEffect(() => {
    let stop: (() => void) | undefined
    let cancelled = false
    void on<GameState>('game-state', (s) => {
      if (s.running && settings.afterLaunch === 'minimize') void minimizeWindow()
      if (!s.running && s.exit?.hint) toast(`${instances?.find((i) => i.id === s.instance)?.name ?? 'Minecraft'} stopped. ${s.exit.hint}`, 'error')
    }).then((fn) => (cancelled ? fn() : (stop = fn)))
    return () => {
      cancelled = true
      stop?.()
    }
  }, [settings.afterLaunch, instances])

  const current = instances?.find((i) => i.id === selected) ?? null
  const openInstance = (id: string) => {
    setSelected(id)
    setTab('play')
  }
  const newInstance = (initial?: string) => setCreating({ initial })
  const working = tasks.filter((t) => t.state === 'running').length

  // A Modrinth modpack becomes an instance; its mods download, then the game.
  const importPack = useCallback(async (path?: string) => {
    try {
      const file = path ?? (await api.pickModpack())
      if (!file) return
      const created = await api.importModpack(file)
      await live.refreshInstances()
      if (!settings.firstRunDone) void saveSettings({ firstRunDone: true })
      setSelected(created.id)
      setTab('play')
      toast(`Importing ${created.name}. Its mods download first, then Minecraft.`)
    } catch (e) {
      toastError(e)
    }
  }, [settings.firstRunDone, saveSettings])

  // .mrpack files dropped anywhere on the window are imported. Other files
  // go to the open instance's content tab, which takes them itself.
  useEffect(() => {
    if (!isTauri()) return
    let stop: (() => void) | undefined
    let cancelled = false
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type !== 'drop') return
        for (const path of event.payload.paths.filter(isModpack)) void importPack(path)
      })
      .then((fn) => (cancelled ? fn() : (stop = fn)))
    return () => {
      cancelled = true
      stop?.()
    }
  }, [importPack])

  if (instances === null) {
    return (
      <div className="grid h-full place-items-center">
        <Busy className="size-6" />
      </div>
    )
  }

  const firstRun = !settings.firstRunDone && instances.length === 0
  return (
    <div className="flex h-full flex-col bg-background text-foreground">
      <TitleBar right={<ProfileMenu onAdd={() => setAddingProfile(true)} />} />
      {firstRun ? (
        <main className="min-h-0 grow">
          <Welcome
            versions={versions}
            onInstall={(v) => newInstance(v)}
            onBrowse={() => {
              void saveSettings({ firstRunDone: true })
              setTab('versions')
            }}
            onSkip={() => void saveSettings({ firstRunDone: true })}
          />
        </main>
      ) : (
        <>
          <NavBar
            current={tab}
            onChange={setTab}
            badges={{ downloads: working || undefined }}
            right={
              <Button onClick={() => newInstance()}>
                <Plus className="size-3.5" /> New instance
              </Button>
            }
          />
          <main className="relative flex min-h-0 grow">
            {tab === 'play' ? (
              <>
                <Sidebar instances={instances} selected={current?.id ?? null} onSelect={setSelected} onNew={() => newInstance()} />
                <div key={current?.id ?? 'home'} className="kryo-fade min-w-0 grow">
                  {current ? (
                    <InstancePage instance={current} onJoin={() => setJoining(current)} onDeleted={() => setSelected(null)} />
                  ) : (
                    <Home instances={instances} versions={versions} onOpen={openInstance} onNew={newInstance} onImport={() => void importPack()} onBrowse={() => setTab('versions')} onJoin={setJoining} />
                  )}
                </div>
              </>
            ) : tab === 'versions' ? (
              <div className="kryo-fade min-w-0 grow">
                <VersionsPage versions={versions} error={versionsError} onRefresh={() => loadVersions(true)} onCreate={(v) => newInstance(v.id)} />
              </div>
            ) : tab === 'downloads' ? (
              <div className="kryo-fade min-w-0 grow">
                <DownloadsPage />
              </div>
            ) : (
              <div className="kryo-fade min-w-0 grow">
                <SettingsPage onAddProfile={() => setAddingProfile(true)} />
              </div>
            )}
            {settings.mobs ? <Mobs count={settings.mobCount} friendlyOnly={settings.friendlyOnly} /> : null}
          </main>
          <BottomBar instances={instances} onDownloads={() => setTab('downloads')} onOpen={openInstance} />
        </>
      )}

      {addingProfile ? <ProfileDialog onClose={() => setAddingProfile(false)} /> : null}
      {firstProfile ? <ProfileDialog first onClose={() => setFirstProfile(null)} onAdded={firstProfile.then} /> : null}
      {creating && versions ? (
        <NewInstanceDialog
          versions={versions}
          initial={creating.initial}
          onClose={() => setCreating(null)}
          onCreated={(i) => {
            setCreating(null)
            if (!settings.firstRunDone) void saveSettings({ firstRunDone: true })
            openInstance(i.id)
          }}
        />
      ) : null}
      {joining ? <JoinDialog instance={joining} onClose={() => setJoining(null)} /> : null}
      <UpdatePrompt busy={working} />
    </div>
  )
}
