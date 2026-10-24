import { useState } from 'react'
import { Server } from 'lucide-react'
import { Button, Caption, Modal, inputCls } from '@/ui'
import { toastError } from '@/ui/Toasts'
import { api, type Instance } from '@/lib/api'
import { activeAccount, useAccounts } from '@/lib/store'

/** Kryoto's own server, first in the list. */
const KRYOTO_SERVER = { name: 'Kryo.to Anarchy', address: 'play.kryo.to' }
const RECENT_KEY = 'kryoto.servers'

function recentServers(): string[] {
  try {
    return JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]') as string[]
  } catch {
    return []
  }
}

function remember(address: string) {
  try {
    const list = [address, ...recentServers().filter((a) => a !== address)].slice(0, 6)
    localStorage.setItem(RECENT_KEY, JSON.stringify(list))
  } catch {
    /* not important */
  }
}

/** Start the game straight into a server. */
export function JoinDialog({ instance, onClose }: { instance: Instance; onClose: () => void }) {
  const [address, setAddress] = useState('')
  const accounts = useAccounts()
  const offline = activeAccount(accounts)?.kind === 'offline'
  const recent = recentServers().filter((a) => a !== KRYOTO_SERVER.address)
  const join = async (to: string) => {
    const target = to.trim()
    if (!target) return
    remember(target)
    onClose()
    try {
      await api.launch(instance.id, target)
    } catch (e) {
      toastError(e)
    }
  }
  return (
    <Modal
      title="Join a server"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!address.trim()} onClick={() => void join(address)}>
            Play and join
          </Button>
        </>
      }
    >
      <div className="grid gap-4">
        <button type="button" onClick={() => void join(KRYOTO_SERVER.address)} className="kryo-radius flex items-center gap-3 border border-border bg-background p-3 text-left transition-colors hover:border-foreground">
          <span className="kryo-pill grid size-9 place-items-center bg-secondary">
            <Server className="size-4" />
          </span>
          <span className="grid">
            <span className="text-xs font-bold">{KRYOTO_SERVER.name}</span>
            <span className="text-[11px] text-muted-foreground">{KRYOTO_SERVER.address}</span>
          </span>
        </button>
        {recent.length ? (
          <div className="grid gap-1.5">
            <Caption>Recent</Caption>
            <div className="flex flex-wrap gap-1.5">
              {recent.map((a) => (
                <Button key={a} size="sm" onClick={() => void join(a)}>
                  {a}
                </Button>
              ))}
            </div>
          </div>
        ) : null}
        <label className="grid gap-2">
          <Caption>Server address</Caption>
          <input autoFocus value={address} onChange={(e) => setAddress(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && void join(address)} placeholder="mc.example.com or 1.2.3.4:25565" className={inputCls} />
        </label>
        <p className="text-[11px] leading-relaxed text-muted-foreground">
          {offline
            ? 'You are playing with an offline profile: servers that check accounts will turn you away. Offline-mode servers let you in.'
            : 'The game starts and connects by itself. Versions before 1.20 need the port when the server is not on 25565.'}
        </p>
      </div>
    </Modal>
  )
}
