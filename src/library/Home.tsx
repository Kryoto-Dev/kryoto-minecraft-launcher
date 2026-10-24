import { Layers, Plus, Server } from 'lucide-react'
import { Button, Caption, Label } from '@/ui'
import { BlockIcon, blockFor } from '@/ui/BlockIcon'
import { formatLastPlayed, formatPlaytime } from '@/lib/format'
import { instanceLabel, type Instance, type VersionList } from '@/lib/api'
import { PlayButton } from './PlayButton'

/** The Play tab with nothing picked: carry on, or start something. */
export function Home({
  instances,
  versions,
  onOpen,
  onNew,
  onBrowse,
  onJoin,
}: {
  instances: Instance[]
  versions: VersionList | null
  onOpen: (id: string) => void
  onNew: (gameVersion?: string) => void
  onBrowse: () => void
  onJoin: (instance: Instance) => void
}) {
  const recent = [...instances].sort((a, b) => (b.lastPlayed ?? b.created) - (a.lastPlayed ?? a.created))
  const last = recent[0]

  if (!last) {
    return (
      <div className="grid h-full place-items-center p-8">
        <div className="kryo-in grid max-w-md justify-items-center gap-5 text-center">
          <BlockIcon name="grass" className="size-20" build glint />
          <div className="grid gap-2">
            <h1 className="text-xl font-bold">Pick a Minecraft to play</h1>
            <p className="text-xs leading-relaxed text-muted-foreground">
              An instance is one Minecraft version with its own worlds, mods and settings. Start with the newest release, or pick any version back to the very first.
            </p>
          </div>
          <div className="flex flex-wrap justify-center gap-2">
            {versions ? (
              <Button variant="primary" size="lg" onClick={() => onNew(versions.latestRelease)}>
                <Plus className="size-4" /> Minecraft {versions.latestRelease}
              </Button>
            ) : null}
            <Button size="lg" onClick={onBrowse}>
              <Layers className="size-4" /> All versions
            </Button>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="grid content-start gap-8 overflow-y-auto p-6">
      <section className="kryo-in kryo-radius relative overflow-hidden border border-border bg-card">
        <div aria-hidden className="absolute -right-10 -top-16 opacity-[0.16]">
          <BlockIcon name={last.icon ?? blockFor(last.id)} className="size-[340px]" />
        </div>
        <div className="hero-side absolute inset-0" aria-hidden />
        <div className="relative grid gap-5 p-8">
          <Caption>
            Continue · {formatLastPlayed(last.lastPlayed)}
          </Caption>
          <button type="button" onClick={() => onOpen(last.id)} className="kryo-square w-fit text-left text-3xl font-bold hover:underline">
            {last.name}
          </button>
          <div className="flex flex-wrap items-center gap-4">
            <PlayButton instance={last} />
            <Caption>{instanceLabel(last)}</Caption>
            {last.playSeconds > 0 ? <Caption>{formatPlaytime(last.playSeconds)} played</Caption> : null}
            {last.installed ? (
              <Button size="sm" variant="ghost" onClick={() => onJoin(last)}>
                <Server className="size-3" /> Join a server
              </Button>
            ) : null}
          </div>
        </div>
      </section>

      {recent.length > 1 ? (
        <section className="grid gap-3">
          <Label>Recent</Label>
          <div className="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-3">
            {recent.slice(1, 9).map((i) => (
              <button key={i.id} type="button" onClick={() => onOpen(i.id)} className="kryo-radius group flex items-center gap-3 border border-border bg-card p-3 text-left transition-colors hover:border-foreground/60">
                <BlockIcon name={i.icon ?? blockFor(i.id)} className="size-10 transition-transform group-hover:-translate-y-0.5" />
                <span className="grid min-w-0">
                  <span className="truncate text-xs font-bold">{i.name}</span>
                  <span className="truncate text-[10px] uppercase tracking-wider text-muted-foreground">
                    {instanceLabel(i)} · {formatLastPlayed(i.lastPlayed)}
                  </span>
                </span>
              </button>
            ))}
          </div>
        </section>
      ) : null}

      <section className="grid gap-3">
        <Label>Start something new</Label>
        <div className="flex flex-wrap gap-2">
          {versions ? (
            <Button onClick={() => onNew(versions.latestRelease)}>
              <Plus className="size-3.5" /> Minecraft {versions.latestRelease}
            </Button>
          ) : null}
          {versions && versions.latestSnapshot !== versions.latestRelease ? (
            <Button onClick={() => onNew(versions.latestSnapshot)}>
              <Plus className="size-3.5" /> Snapshot {versions.latestSnapshot}
            </Button>
          ) : null}
          <Button onClick={onBrowse}>
            <Layers className="size-3.5" /> All versions
          </Button>
        </div>
      </section>
    </div>
  )
}
