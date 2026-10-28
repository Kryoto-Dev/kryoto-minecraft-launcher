import { useEffect, useState } from 'react'
import { FolderOpen, Server } from 'lucide-react'
import { Button, Caption, IconButton } from '@/ui'
import { BlockIcon, blockFor } from '@/ui/BlockIcon'
import { toastError } from '@/ui/Toasts'
import { formatLastPlayed, formatPlaytime } from '@/lib/format'
import { api, instanceLabel, type ContentKind, type Instance } from '@/lib/api'
import { taskFor, taskFraction, useGames, useTasks } from '@/lib/store'
import { cn } from '@/lib/utils'
import { ContentTab } from './ContentTab'
import { LogTab } from './LogTab'
import { InstanceSettings } from './InstanceSettings'
import { PlayButton } from './PlayButton'

type Section = ContentKind | 'log' | 'settings'

const SECTIONS: { id: Section; label: string; modded?: boolean }[] = [
  { id: 'mod', label: 'Mods', modded: true },
  { id: 'resourcepack', label: 'Resource packs' },
  { id: 'shaderpack', label: 'Shaders', modded: true },
  { id: 'world', label: 'Worlds' },
  { id: 'log', label: 'Log' },
  { id: 'settings', label: 'Settings' },
]

export function InstancePage({
  instance,
  showLog,
  onLogShown,
  onJoin,
  onDeleted,
}: {
  instance: Instance
  showLog: boolean
  onLogShown: () => void
  onJoin: () => void
  onDeleted: () => void
}) {
  const modded = instance.loader.kind !== 'vanilla'
  const [section, setSection] = useState<Section>(modded ? 'mod' : 'resourcepack')
  const games = useGames()
  const game = games[instance.id]
  const task = taskFor(useTasks(), instance.id)
  // A new instance's block fills in with its download.
  const filling = task && !instance.installed ? (taskFraction(task) ?? 0) : null

  useEffect(() => setSection(instance.loader.kind !== 'vanilla' ? 'mod' : 'resourcepack'), [instance.id, instance.loader.kind])
  // A crash jumps to the log, where the reason is; so does "Show the log".
  useEffect(() => {
    if (game?.exit?.hint) setSection('log')
  }, [game?.exit])
  useEffect(() => {
    if (!showLog) return
    setSection('log')
    onLogShown()
  }, [showLog, onLogShown])

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="kryo-in relative shrink-0 overflow-hidden border-b border-border">
        <div aria-hidden className="absolute -right-6 -top-24 opacity-[0.12]">
          <BlockIcon name={instance.icon ?? blockFor(instance.id)} className="size-[320px]" />
        </div>
        <div className="hero-side absolute inset-0" aria-hidden />
        <div className="relative flex items-center gap-6 px-6 py-6">
          <BlockIcon key={instance.id} name={instance.icon ?? blockFor(instance.id)} className="size-20" build progress={filling} glint={!!game?.running} />
          <div className="grid min-w-0 gap-3">
            <div className="grid gap-1">
              <h1 className="truncate text-2xl font-bold">{instance.name}</h1>
              <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
                <Caption className="text-foreground/80">{instanceLabel(instance)}</Caption>
                {instance.loader.version ? <Caption>Loader {instance.loader.version}</Caption> : null}
                {instance.playSeconds > 0 ? <Caption>{formatPlaytime(instance.playSeconds)} played</Caption> : !instance.lastPlayed ? <Caption>Not played yet</Caption> : null}
                {instance.lastPlayed ? <Caption>Last played {formatLastPlayed(instance.lastPlayed).toLowerCase()}</Caption> : null}
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-3">
              <PlayButton instance={instance} />
              {instance.installed && !game?.running ? (
                <Button onClick={onJoin}>
                  <Server className="size-3.5" /> Join a server
                </Button>
              ) : null}
              <IconButton label="Open the game folder" onClick={() => void api.openFolder(instance.id).catch(toastError)}>
                <FolderOpen className="size-3.5" />
              </IconButton>
            </div>
          </div>
        </div>
        <nav aria-label="Instance" className="relative flex gap-1 px-5">
          {SECTIONS.map((s) => (
            <button
              key={s.id}
              type="button"
              aria-current={section === s.id}
              onClick={() => setSection(s.id)}
              className={cn(
                'kryo-square relative h-10 px-3 text-[11px] font-bold uppercase tracking-[0.18em] transition-colors',
                section === s.id ? 'text-foreground after:absolute after:inset-x-3 after:bottom-0 after:h-0.5 after:bg-primary' : 'text-muted-foreground hover:text-foreground',
                s.modded && !modded && 'opacity-60',
              )}
            >
              {s.label}
              {s.id === 'log' && game?.running ? <span className="ml-1.5 inline-block size-1.5 rounded-full bg-success align-middle" /> : null}
            </button>
          ))}
        </nav>
      </header>
      <div className="min-h-0 grow overflow-hidden">
        {section === 'log' ? (
          <LogTab instance={instance} />
        ) : section === 'settings' ? (
          <InstanceSettings instance={instance} onDeleted={onDeleted} />
        ) : (
          <ContentTab key={`${instance.id}-${section}`} instance={instance} kind={section} />
        )}
      </div>
    </div>
  )
}
