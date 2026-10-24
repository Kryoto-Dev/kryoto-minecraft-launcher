import { useState } from 'react'
import { ArrowRight, Layers, Plus } from 'lucide-react'
import { Button, Caption, inputCls } from '@/ui'
import { KryoMark } from '@/ui/ascii/KryoMark'
import { BlockIcon } from '@/ui/BlockIcon'
import { api, type VersionList } from '@/lib/api'
import { errorText } from '@/lib/bridge'
import { activeProfile, live, useProfiles } from '@/lib/store'
import { cn } from '@/lib/utils'

/**
 * The first start: a name to play under, then something to play. Two steps,
 * both skippable, and never shown again once done.
 */
export function Welcome({ versions, onInstall, onBrowse, onSkip }: { versions: VersionList | null; onInstall: (version: string) => void; onBrowse: () => void; onSkip: () => void }) {
  const profiles = useProfiles()
  const profile = activeProfile(profiles)
  const [name, setName] = useState('')
  const [error, setError] = useState<string | null>(null)
  const valid = /^[A-Za-z0-9_]{3,16}$/.test(name)
  const add = async () => {
    try {
      await api.addProfile(name)
      await live.refreshProfiles()
    } catch (e) {
      setError(errorText(e))
    }
  }

  return (
    <div className="grid h-full place-items-center overflow-y-auto p-8">
      <div className="kryo-in grid w-full max-w-lg justify-items-center gap-8 text-center">
        <div className="grid justify-items-center gap-5">
          <div className="flex items-center gap-4">
            <BlockIcon name="grass" className="size-14" build glint />
            <KryoMark mode="reveal" className="h-7" />
          </div>
          <div className="grid gap-2">
            <h1 className="text-2xl font-bold">Minecraft, every version</h1>
            <p className="text-xs leading-relaxed text-muted-foreground">From today's release back to the first alpha, with Fabric, Forge and mods a click away. Java and the game files download by themselves.</p>
          </div>
        </div>

        {!profile ? (
          <div className="grid w-full gap-3">
            <Caption>1 · Your name in game</Caption>
            <div className="flex gap-2">
              <input
                autoFocus
                value={name}
                maxLength={16}
                placeholder="Steve"
                onChange={(e) => {
                  setName(e.target.value.replace(/\s/g, ''))
                  setError(null)
                }}
                onKeyDown={(e) => e.key === 'Enter' && valid && void add()}
                className={cn(inputCls, 'h-12 text-center text-sm')}
              />
              <Button variant="primary" size="lg" disabled={!valid} onClick={() => void add()} aria-label="Continue">
                <ArrowRight className="size-4" />
              </Button>
            </div>
            <p className={cn('text-[11px] leading-relaxed', error ? 'text-destructive' : 'text-muted-foreground')}>
              {error ?? '3 to 16 letters, numbers or _. You play singleplayer and offline-mode servers, like play.kryo.to.'}
            </p>
          </div>
        ) : (
          <div className="grid w-full gap-3">
            <Caption>2 · What to play, {profile.name}</Caption>
            <div className="grid gap-2">
              {versions ? (
                <Button variant="primary" size="lg" onClick={() => onInstall(versions.latestRelease)}>
                  <Plus className="size-4" /> Minecraft {versions.latestRelease}
                </Button>
              ) : null}
              <Button size="lg" onClick={onBrowse}>
                <Layers className="size-4" /> Pick another version
              </Button>
            </div>
          </div>
        )}
        <button type="button" onClick={onSkip} className="kryo-square text-[10px] uppercase tracking-[0.2em] text-muted-foreground hover:text-foreground">
          Skip
        </button>
      </div>
    </div>
  )
}
