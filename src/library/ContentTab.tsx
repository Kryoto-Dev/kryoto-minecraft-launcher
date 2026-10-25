import { useCallback, useEffect, useMemo, useState } from 'react'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { Compass, FilePlus2, FolderOpen, Puzzle, Search, Trash2 } from 'lucide-react'
import { Busy, Button, Check, IconButton, Modal } from '@/ui'
import { toast, toastError } from '@/ui/Toasts'
import { api, loaderLabel, type ContentItem, type ContentKind, type Instance } from '@/lib/api'
import { errorText, isTauri } from '@/lib/bridge'
import { useTasks } from '@/lib/store'
import { bytes } from '@/lib/units'
import { formatLastPlayed } from '@/lib/format'
import { isModpack } from '@/lib/modpack'
import { cn } from '@/lib/utils'
import { ModrinthBrowser } from './ModrinthBrowser'

const WORDS: Record<ContentKind, { one: string; many: string; folder: string }> = {
  mod: { one: 'mod', many: 'mods', folder: 'mods' },
  resourcepack: { one: 'resource pack', many: 'resource packs', folder: 'resourcepacks' },
  shaderpack: { one: 'shader pack', many: 'shader packs', folder: 'shaderpacks' },
  world: { one: 'world', many: 'worlds', folder: 'saves' },
}

export function ContentTab({ instance, kind }: { instance: Instance; kind: ContentKind }) {
  const [items, setItems] = useState<ContentItem[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [browsing, setBrowsing] = useState(false)
  const [confirm, setConfirm] = useState<ContentItem | null>(null)
  const [dropping, setDropping] = useState(false)
  const tasks = useTasks()
  const words = WORDS[kind]
  const vanilla = instance.loader.kind === 'vanilla'
  const needsLoader = vanilla && kind === 'mod'
  const oldPacks = /^(b|a|c|rd-|inf-)/.test(instance.gameVersion) || /^1\.[0-5](\.|$)/.test(instance.gameVersion)
  const folder = kind === 'resourcepack' && oldPacks ? 'texturepacks' : words.folder

  const load = useCallback(() => {
    api
      .content(instance.id, kind)
      .then((list) => {
        setItems(list)
        setError(null)
      })
      .catch((e) => setError(errorText(e)))
  }, [instance.id, kind])
  useEffect(load, [load])

  // Reload when a Modrinth job for this instance ends.
  const finished = tasks.filter((t) => t.instance === instance.id && t.state === 'done').length
  useEffect(() => {
    if (finished) load()
  }, [finished, load])

  // Files dropped on the window go into this instance.
  useEffect(() => {
    if (!isTauri() || kind === 'world') return
    let stop: (() => void) | undefined
    let cancelled = false
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload
        if (p.type === 'enter' || p.type === 'over') setDropping(true)
        else if (p.type === 'leave') setDropping(false)
        else if (p.type === 'drop') {
          setDropping(false)
          // Modpacks become instances of their own (App takes them).
          const files = p.paths.filter((f) => !isModpack(f))
          if (!files.length) return
          void api
            .importContent(instance.id, files)
            .then((done) => {
              toast(done.length === 1 ? `Added ${done[0]!.file}.` : `Added ${done.length} files.`)
              load()
            })
            .catch(toastError)
        }
      })
      .then((fn) => (cancelled ? fn() : (stop = fn)))
    return () => {
      cancelled = true
      stop?.()
    }
  }, [instance.id, kind, load])

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase()
    return (items ?? []).filter((i) => !q || i.name.toLowerCase().includes(q) || i.file.toLowerCase().includes(q))
  }, [items, query])

  const toggle = async (item: ContentItem, enabled: boolean) => {
    try {
      await api.setContentEnabled(instance.id, kind, item.file, enabled)
      load()
    } catch (e) {
      toastError(e)
    }
  }
  const remove = async (item: ContentItem) => {
    setConfirm(null)
    try {
      await api.deleteContent(instance.id, kind, item.file)
      load()
    } catch (e) {
      toastError(e)
    }
  }

  return (
    <div className={cn('relative flex h-full min-h-0 flex-col', dropping && 'outline-2 -outline-offset-8 outline-dashed outline-foreground/50')}>
      <div className="flex shrink-0 flex-wrap items-center gap-2 px-6 py-3">
        <label className="kryo-pill flex h-9 w-64 items-center gap-2 border border-border px-3 focus-within:border-foreground">
          <Search className="size-3.5 text-muted-foreground" />
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={`Filter ${words.many}`} className="min-w-0 grow bg-transparent text-xs outline-none placeholder:text-muted-foreground" />
        </label>
        <span className="text-[10px] uppercase tracking-wider text-muted-foreground">{items ? `${items.length} ${items.length === 1 ? words.one : words.many}` : ''}</span>
        <div className="ml-auto flex items-center gap-2">
          <IconButton label={`Open the ${folder} folder`} onClick={() => void api.openFolder(instance.id, folder).catch(toastError)}>
            <FolderOpen className="size-3.5" />
          </IconButton>
          {kind !== 'world' ? (
            <>
              <Button disabled={needsLoader} onClick={() => void api.pickContent(instance.id, kind).then((d) => d.length && load()).catch(toastError)}>
                <FilePlus2 className="size-3.5" /> Add files
              </Button>
              <Button variant="primary" disabled={needsLoader} onClick={() => setBrowsing(true)}>
                <Compass className="size-3.5" /> Browse Modrinth
              </Button>
            </>
          ) : null}
        </div>
      </div>

      <div className="min-h-0 grow overflow-y-auto px-6 pb-6">
        {needsLoader ? (
          <Empty
            icon={<Puzzle className="size-6" />}
            title="Vanilla doesn't run mods"
            text="Pick Fabric, Quilt, Forge or NeoForge for this instance in its Settings, and mods for it show up here. Your worlds stay where they are."
          />
        ) : error ? (
          <p role="alert" className="py-6 text-xs text-destructive">
            {error}
          </p>
        ) : !items ? (
          <div className="grid place-items-center py-16">
            <Busy className="size-5" />
          </div>
        ) : items.length === 0 ? (
          <Empty
            icon={<Puzzle className="size-6" />}
            title={`No ${words.many} yet`}
            text={
              kind === 'world'
                ? 'Worlds you make in this instance show up here.'
                : kind === 'shaderpack'
                  ? 'Shader packs need a shader mod (Iris or Oculus) in Mods. Browse Modrinth, or drop .zip files on this window.'
                  : `Browse Modrinth for ${words.many} that fit ${vanilla ? 'Minecraft' : loaderLabel(instance.loader.kind)} ${instance.gameVersion}, or drop files on this window.`
            }
          />
        ) : (
          <ul className="grid gap-1">
            {shown.map((item) => (
              <li key={item.file} className={cn('kryo-radius group flex items-center gap-3 border border-transparent px-3 py-2 transition-colors hover:border-border hover:bg-card', !item.enabled && 'opacity-55')}>
                {item.icon ? (
                  <img src={item.icon} alt="" className="kryo-square size-9 shrink-0 rounded-[4px] object-cover [image-rendering:pixelated]" />
                ) : (
                  <span className="kryo-radius grid size-9 shrink-0 place-items-center bg-secondary text-muted-foreground">
                    <Puzzle className="size-4" />
                  </span>
                )}
                <div className="grid min-w-0 grow">
                  <span className="flex items-baseline gap-2">
                    <span className="truncate text-xs font-bold">{item.name}</span>
                    {item.version ?? item.source?.versionNumber ? <span className="shrink-0 text-[10px] text-muted-foreground">{item.version ?? item.source?.versionNumber}</span> : null}
                  </span>
                  <span className="truncate text-[11px] text-muted-foreground">
                    {item.description ?? (kind === 'world' ? `Played ${formatLastPlayed(item.modified).toLowerCase()}` : item.file)}
                  </span>
                </div>
                {item.size ? <span className="hidden shrink-0 text-[10px] tabular-nums text-muted-foreground lg:block">{bytes(item.size)}</span> : null}
                {kind === 'mod' || kind === 'resourcepack' || kind === 'shaderpack' ? (
                  <Check checked={item.enabled} onChange={(on) => void toggle(item, on)} label={<span className="sr-only">{item.enabled ? 'On' : 'Off'}</span>} />
                ) : null}
                {kind !== 'world' ? (
                  <IconButton label={`Delete ${item.name}`} className="opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100" onClick={() => setConfirm(item)}>
                    <Trash2 className="size-3.5" />
                  </IconButton>
                ) : null}
              </li>
            ))}
          </ul>
        )}
        {kind === 'resourcepack' && items?.length ? <p className="pt-4 text-[11px] text-muted-foreground">Turn packs on in game: Options, Resource Packs.</p> : null}
      </div>

      {browsing ? <ModrinthBrowser instance={instance} kind={kind} installed={items ?? []} onClose={() => setBrowsing(false)} /> : null}
      {confirm ? (
        <Modal
          title={`Delete ${words.one}`}
          onClose={() => setConfirm(null)}
          footer={
            <>
              <Button onClick={() => setConfirm(null)}>Keep it</Button>
              <Button variant="danger" onClick={() => void remove(confirm)}>
                Delete
              </Button>
            </>
          }
        >
          <p className="text-xs leading-relaxed">
            Delete <b>{confirm.name}</b> ({confirm.file}) from this instance? The file is removed from its {folder} folder.
          </p>
        </Modal>
      ) : null}
    </div>
  )
}

function Empty({ icon, title, text }: { icon: React.ReactNode; title: string; text: string }) {
  return (
    <div className="grid justify-items-center gap-3 py-16 text-center">
      <span className="kryo-pill grid size-12 place-items-center border border-border text-muted-foreground">{icon}</span>
      <h3 className="text-sm font-bold">{title}</h3>
      <p className="max-w-md text-xs leading-relaxed text-muted-foreground">{text}</p>
    </div>
  )
}
