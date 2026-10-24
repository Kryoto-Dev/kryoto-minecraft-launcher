import { Layers, LogIn, Plus, UserRound } from 'lucide-react'
import { Button, Caption } from '@/ui'
import { KryoMark } from '@/ui/ascii/KryoMark'
import { BlockIcon } from '@/ui/BlockIcon'
import type { VersionList } from '@/lib/api'
import { activeAccount, useAccounts } from '@/lib/store'
import type { AccountDialog } from '@/accounts/Accounts'

/**
 * The first start: who plays, then what. Two steps, both skippable, and
 * never shown again once done.
 */
export function Welcome({
  microsoft,
  versions,
  onAccount,
  onInstall,
  onBrowse,
  onSkip,
}: {
  microsoft: boolean
  versions: VersionList | null
  onAccount: (d: AccountDialog) => void
  onInstall: (version: string) => void
  onBrowse: () => void
  onSkip: () => void
}) {
  const accounts = useAccounts()
  const account = activeAccount(accounts)

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

        {!account ? (
          <div className="grid w-full gap-3">
            <Caption>1 · Who's playing</Caption>
            <div className="grid gap-2">
              <Button variant="primary" size="lg" disabled={!microsoft} onClick={() => onAccount('microsoft')}>
                <LogIn className="size-4" /> Sign in with Microsoft
              </Button>
              <Button size="lg" onClick={() => onAccount('offline')}>
                <UserRound className="size-4" /> Play offline
              </Button>
            </div>
            <p className="text-[11px] leading-relaxed text-muted-foreground">
              {microsoft
                ? 'A Microsoft account that owns Minecraft plays everywhere. An offline profile plays singleplayer and offline-mode servers.'
                : 'This build has no Microsoft sign-in; offline profiles play singleplayer and offline-mode servers.'}
            </p>
          </div>
        ) : (
          <div className="grid w-full gap-3">
            <Caption>2 · What to play, {account.name}</Caption>
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
