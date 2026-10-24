import { useState } from 'react'
import { Check, ChevronDown, Plus, UserRound } from 'lucide-react'
import { Button, Caption, MenuButton, Modal, inputCls, type MenuEntry } from '@/ui'
import { toast, toastError } from '@/ui/Toasts'
import { api, type Profile } from '@/lib/api'
import { errorText } from '@/lib/bridge'
import { activeProfile, live, useProfiles } from '@/lib/store'

/** A profile's letter, in the pill Kryoto Desktop draws for an avatar-less account. */
export function Avatar({ profile, className = 'size-6' }: { profile: Profile | null; className?: string }) {
  return <span className={`kryo-pill grid ${className} place-items-center bg-secondary text-[10px] font-bold`}>{(profile?.name ?? '?').slice(0, 1).toUpperCase()}</span>
}

/** The profile pill in the title bar, and its menu. */
export function ProfileMenu({ onAdd }: { onAdd: () => void }) {
  const profiles = useProfiles()
  const current = activeProfile(profiles)
  const items: MenuEntry[] = [
    ...(profiles.profiles.length ? [{ heading: 'Play as' } as MenuEntry] : []),
    ...profiles.profiles.map((p) => ({
      label: p.name,
      checked: p.id === profiles.active,
      icon: p.id === profiles.active ? <Check /> : <UserRound />,
      onSelect: () => void api.setActiveProfile(p.id).then(live.refreshProfiles).catch(toastError),
    })),
    ...(profiles.profiles.length ? [{ separator: true } as MenuEntry] : []),
    { label: 'New profile', icon: <Plus />, onSelect: onAdd },
    ...(current
      ? [
          { separator: true } as MenuEntry,
          {
            label: `Remove ${current.name}`,
            danger: true,
            onSelect: () =>
              void api
                .removeProfile(current.id)
                .then(live.refreshProfiles)
                .then(() => toast(`${current.name} removed. Worlds made with it stay.`))
                .catch(toastError),
          },
        ]
      : []),
  ]
  return (
    <MenuButton
      label="Profile"
      align="right"
      items={items}
      className="kryo-pill flex h-7 items-center gap-2 border border-border pl-0.5 pr-2.5 text-[11px] text-foreground transition-colors hover:border-foreground aria-expanded:border-foreground"
      trigger={
        <>
          {current ? <Avatar profile={current} /> : <span className="kryo-pill grid size-6 place-items-center bg-secondary"><UserRound className="size-3" /></span>}
          <span className="max-w-40 truncate">{current?.name ?? 'Pick a name'}</span>
          <ChevronDown className="size-3 text-muted-foreground" />
        </>
      }
    />
  )
}

/** A new profile: the name to play under. `first`: asked when a game starts and there is none yet. */
export function ProfileDialog({ onClose, onAdded, first = false }: { onClose: () => void; onAdded?: () => void; first?: boolean }) {
  const [name, setName] = useState('')
  const [error, setError] = useState<string | null>(null)
  const valid = /^[A-Za-z0-9_]{3,16}$/.test(name)
  const add = async () => {
    try {
      const profile = await api.addProfile(name)
      await live.refreshProfiles()
      toast(`Playing as ${profile.name}.`)
      onClose()
      onAdded?.()
    } catch (e) {
      setError(errorText(e))
    }
  }
  return (
    <Modal
      title={first ? "Your name in game" : "New profile"}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!valid} onClick={() => void add()}>
            {first ? 'Save and play' : 'Add profile'}
          </Button>
        </>
      }
    >
      <div className="grid gap-4">
        {first ? <p className="text-xs leading-relaxed text-foreground">Pick the name you play under. It's saved as your profile, and you can add more later.</p> : null}
        <label className="grid gap-2">
          <Caption>Name in game</Caption>
          <input
            autoFocus
            value={name}
            maxLength={16}
            onChange={(e) => {
              setName(e.target.value.replace(/\s/g, ''))
              setError(null)
            }}
            onKeyDown={(e) => e.key === 'Enter' && valid && void add()}
            placeholder="Steve"
            className={inputCls}
          />
        </label>
        <p className="text-[11px] text-muted-foreground">3 to 16 letters, numbers or _.</p>
        {error ? (
          <p role="alert" className="text-xs text-destructive">
            {error}
          </p>
        ) : null}
        <p className="text-xs leading-relaxed text-muted-foreground">
          A profile plays singleplayer and servers that run in offline mode, like play.kryo.to. Servers that check Microsoft accounts, Realms and skins aren't available.
          If you enjoy the game, buy it at minecraft.net.
        </p>
      </div>
    </Modal>
  )
}
