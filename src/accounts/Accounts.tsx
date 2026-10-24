import { useEffect, useRef, useState } from 'react'
import { ArrowUpRight, Check, ChevronDown, Copy, LogIn, UserRound } from 'lucide-react'
import { Button, Caption, MenuButton, Modal, inputCls, type MenuEntry } from '@/ui'
import { Matrix } from '@/ui/Matrix'
import { toast, toastError } from '@/ui/Toasts'
import { api, type Account, type DeviceCode } from '@/lib/api'
import { errorText } from '@/lib/bridge'
import { activeAccount, live, useAccounts } from '@/lib/store'
import { openExternal } from '@/lib/window'

/** A player's head: Microsoft accounts' skins, a letter for offline profiles. */
export function Avatar({ account, className = 'size-6' }: { account: Account | null; className?: string }) {
  const [failed, setFailed] = useState(false)
  if (account?.kind === 'microsoft' && !failed) {
    return <img src={`https://mc-heads.net/avatar/${account.id}/48`} alt="" onError={() => setFailed(true)} className={`kryo-square ${className} rounded-[3px] [image-rendering:pixelated]`} />
  }
  return (
    <span className={`kryo-pill grid ${className} place-items-center bg-secondary text-[10px] font-bold`}>{(account?.name ?? '?').slice(0, 1).toUpperCase()}</span>
  )
}

export type AccountDialog = 'microsoft' | 'offline' | null

/** The account pill in the title bar, and its menu. */
export function AccountMenu({ onOpen }: { onOpen: (d: AccountDialog) => void }) {
  const accounts = useAccounts()
  const current = activeAccount(accounts)
  const items: MenuEntry[] = [
    ...(accounts.accounts.length ? [{ heading: 'Play as' } as MenuEntry] : []),
    ...accounts.accounts.map((a) => ({
      label: a.name,
      hint: a.kind === 'microsoft' ? 'Microsoft' : 'Offline',
      checked: a.id === accounts.active,
      icon: a.id === accounts.active ? <Check /> : <UserRound />,
      onSelect: () => void api.setActiveAccount(a.id).then(live.refreshAccounts).catch(toastError),
    })),
    ...(accounts.accounts.length ? [{ separator: true } as MenuEntry] : []),
    { label: 'Sign in with Microsoft', icon: <LogIn />, onSelect: () => onOpen('microsoft') },
    { label: 'Add an offline profile', icon: <UserRound />, onSelect: () => onOpen('offline') },
    ...(current
      ? [
          { separator: true } as MenuEntry,
          {
            label: `Remove ${current.name}`,
            danger: true,
            onSelect: () =>
              void api
                .removeAccount(current.id)
                .then(live.refreshAccounts)
                .then(() => toast(`${current.name} removed from this launcher.`))
                .catch(toastError),
          },
        ]
      : []),
  ]
  return (
    <MenuButton
      label="Account"
      align="right"
      items={items}
      className="kryo-pill flex h-7 items-center gap-2 border border-border pl-0.5 pr-2.5 text-[11px] text-foreground transition-colors hover:border-foreground aria-expanded:border-foreground"
      trigger={
        current ? (
          <>
            <Avatar account={current} />
            <span className="max-w-40 truncate">{current.name}</span>
            {current.kind === 'offline' ? <span className="text-[9px] uppercase tracking-wider text-muted-foreground">Offline</span> : null}
            <ChevronDown className="size-3 text-muted-foreground" />
          </>
        ) : (
          <>
            <span className="kryo-pill grid size-6 place-items-center bg-secondary">
              <UserRound className="size-3" />
            </span>
            <span>Add an account</span>
            <ChevronDown className="size-3 text-muted-foreground" />
          </>
        )
      }
    />
  )
}

/**
 * Microsoft's device-code sign-in: the player signs in on microsoft.com in
 * their own browser and types the code shown here. The launcher never sees a
 * password.
 */
export function MicrosoftDialog({ available, onClose }: { available: boolean; onClose: () => void }) {
  const [code, setCode] = useState<DeviceCode | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const closed = useRef(false)

  useEffect(() => {
    if (!available) return
    closed.current = false
    setCode(null)
    setError(null)
    let cancelled = false
    void (async () => {
      try {
        const c = await api.microsoftStart()
        if (cancelled) return
        setCode(c)
        const account = await api.microsoftFinish()
        if (cancelled) return
        await live.refreshAccounts()
        toast(`Signed in as ${account.name}.`)
        onClose()
      } catch (e) {
        if (!cancelled && !closed.current && errorText(e) !== 'Cancelled.') setError(errorText(e))
      }
    })()
    return () => {
      cancelled = true
    }
  }, [available, attempt, onClose])

  const close = () => {
    closed.current = true
    void api.microsoftCancel().catch(() => {})
    onClose()
  }

  return (
    <Modal
      title="Sign in with Microsoft"
      onClose={close}
      footer={
        <>
          <Button onClick={close}>Cancel</Button>
          {error ? (
            <Button variant="primary" onClick={() => setAttempt((n) => n + 1)}>
              Try again
            </Button>
          ) : null}
        </>
      }
    >
      {!available ? (
        <p className="text-xs leading-relaxed text-muted-foreground">
          This build of the launcher has no Microsoft sign-in set up. You can still play with an offline profile. Builds from kryo.to have sign-in.
        </p>
      ) : error ? (
        <p role="alert" className="text-xs leading-relaxed text-destructive">
          {error}
        </p>
      ) : !code ? (
        <div className="flex items-center gap-3 text-xs text-muted-foreground">
          <Matrix state="connect" className="size-4" /> Asking Microsoft for a code
        </div>
      ) : (
        <div className="grid gap-5">
          <p className="text-xs leading-relaxed text-muted-foreground">
            Open the Microsoft page, sign in there, and enter this code. Your password goes to Microsoft only; the launcher never sees it.
          </p>
          <div className="flex items-center justify-center gap-3">
            <span className="kryo-ascii-art select-text text-3xl font-bold tracking-[0.3em] text-foreground">{code.userCode}</span>
            <button
              type="button"
              aria-label="Copy the code"
              title={copied ? 'Copied' : 'Copy'}
              className="kryo-pill grid size-8 place-items-center border border-border text-muted-foreground hover:border-foreground hover:text-foreground"
              onClick={() =>
                void navigator.clipboard.writeText(code.userCode).then(() => {
                  setCopied(true)
                  setTimeout(() => setCopied(false), 1400)
                })
              }
            >
              {copied ? <Check className="size-3.5 text-success" /> : <Copy className="size-3.5" />}
            </button>
          </div>
          <Button variant="primary" size="lg" className="justify-self-center" onClick={() => void navigator.clipboard.writeText(code.userCode).finally(() => openExternal(code.verificationUri))}>
            Open {new URL(code.verificationUri).host} <ArrowUpRight className="size-4" />
          </Button>
          <div className="flex items-center justify-center gap-2 text-[11px] text-muted-foreground">
            <Matrix state="wait" className="size-3" /> Waiting for you to finish in the browser
          </div>
          <p className="text-[11px] leading-relaxed text-muted-foreground">
            The account must own Minecraft: Java Edition. Signing in lets you join servers that check accounts, use your skin and play Realms.
          </p>
        </div>
      )}
    </Modal>
  )
}

/** An offline profile: a name, nothing else. */
export function OfflineDialog({ onClose }: { onClose: () => void }) {
  const [name, setName] = useState('')
  const [error, setError] = useState<string | null>(null)
  const valid = /^[A-Za-z0-9_]{3,16}$/.test(name)
  const add = async () => {
    try {
      const account = await api.addOffline(name)
      await live.refreshAccounts()
      toast(`Playing as ${account.name} (offline).`)
      onClose()
    } catch (e) {
      setError(errorText(e))
    }
  }
  return (
    <Modal
      title="Offline profile"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!valid} onClick={() => void add()}>
            Add profile
          </Button>
        </>
      }
    >
      <div className="grid gap-4">
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
          Offline profiles play singleplayer and servers that run in offline mode. Servers that check accounts, Realms and skins need a Microsoft account that owns
          Minecraft. If you enjoy the game, buy it at minecraft.net.
        </p>
      </div>
    </Modal>
  )
}
