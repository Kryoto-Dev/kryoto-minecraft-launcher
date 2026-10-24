import { useState } from 'react'
import { ArrowUpRight, FolderOpen, LogIn, Trash2, UserRound } from 'lucide-react'
import { Button, Caption, Check, Dropdown, Section, Segmented, inputCls } from '@/ui'
import { KryoMark } from '@/ui/ascii/KryoMark'
import { JavaPicker, MemorySlider, NumberField } from '@/ui/Controls'
import { toastError } from '@/ui/Toasts'
import { api } from '@/lib/api'
import { useApp } from '@/lib/app-context'
import { FONTS, PALETTES, RADII } from '@/lib/look'
import { live, useAccounts } from '@/lib/store'
import { openExternal } from '@/lib/window'
import { cn } from '@/lib/utils'
import { Avatar, type AccountDialog } from '@/accounts/Accounts'
import { UpdateCheck } from '@/shell/UpdatePrompt'

type Pane = 'game' | 'accounts' | 'look' | 'about'

const PANES: [Pane, string][] = [
  ['game', 'Game'],
  ['accounts', 'Accounts'],
  ['look', 'Interface'],
  ['about', 'About'],
]

export function SettingsPage({ onAccount }: { onAccount: (d: AccountDialog) => void }) {
  const [pane, setPane] = useState<Pane>('game')
  return (
    <div className="grid h-full min-h-0 grid-cols-[200px_1fr]">
      <nav aria-label="Settings" className="grid content-start gap-1 border-r border-border p-3">
        {PANES.map(([id, label]) => (
          <button
            key={id}
            type="button"
            aria-current={pane === id}
            onClick={() => setPane(id)}
            className={cn('kryo-pill px-3 py-2 text-left text-[11px] uppercase tracking-wider transition-colors', pane === id ? 'bg-primary font-bold text-primary-foreground' : 'text-muted-foreground hover:bg-secondary hover:text-foreground')}
          >
            {label}
          </button>
        ))}
      </nav>
      <div className="min-h-0 overflow-y-auto">
        <div className="grid max-w-3xl gap-8 px-6 py-6">
          {pane === 'game' ? <GamePane /> : pane === 'accounts' ? <AccountsPane onAccount={onAccount} /> : pane === 'look' ? <LookPane /> : <AboutPane />}
        </div>
      </div>
    </div>
  )
}

function GamePane() {
  const { info, settings, saveSettings } = useApp()
  return (
    <>
      <Section title="Memory" hint="For every instance that doesn't set its own.">
        <MemorySlider value={settings.memoryMb} totalMb={info.totalMemoryMb} onChange={(mb) => void saveSettings({ memoryMb: mb })} />
      </Section>
      <Section title="Java">
        <JavaPicker value={settings.javaPath} onChange={(p) => void saveSettings({ javaPath: p })} />
      </Section>
      <Section title="Game window">
        <div className="flex flex-wrap items-center gap-3">
          <NumberField label="Width" min={320} value={settings.width} onChange={(w) => void saveSettings({ width: w })} />
          <span className="text-muted-foreground">×</span>
          <NumberField label="Height" min={240} value={settings.height} onChange={(h) => void saveSettings({ height: h })} />
          <Check checked={settings.fullscreen} onChange={(f) => void saveSettings({ fullscreen: f })} label="Full screen" />
        </div>
      </Section>
      <Section title="Java arguments" hint="Added to every start. Leave empty unless you know you need something.">
        <input
          defaultValue={settings.jvmArgs}
          spellCheck={false}
          placeholder="-XX:+UseZGC"
          onBlur={(e) => e.target.value.trim() !== settings.jvmArgs && void saveSettings({ jvmArgs: e.target.value.trim() })}
          className={inputCls}
        />
      </Section>
      <Section title="When the game starts">
        <Segmented
          value={settings.afterLaunch}
          onChange={(v) => void saveSettings({ afterLaunch: v })}
          options={[
            { value: 'keep', label: 'Keep the launcher open' },
            { value: 'minimize', label: 'Minimize it' },
          ]}
        />
      </Section>
      <Section title="Downloads" hint="Files downloaded at the same time. Lower it on a slow or shared connection.">
        <Dropdown
          className="w-40"
          value={String(settings.parallelDownloads)}
          onChange={(v) => void saveSettings({ parallelDownloads: Number(v) })}
          options={['4', '8', '16', '32'].map((v) => ({ value: v, label: `${v} at once` }))}
        />
      </Section>
    </>
  )
}

function AccountsPane({ onAccount }: { onAccount: (d: AccountDialog) => void }) {
  const accounts = useAccounts()
  return (
    <>
      <Section title="Accounts" hint="The account the game starts with is the one ticked. Switch any time from the title bar.">
        {accounts.accounts.length === 0 ? <p className="text-xs text-muted-foreground">No accounts yet.</p> : null}
        <ul className="grid gap-1.5">
          {accounts.accounts.map((a) => (
            <li key={a.id} className="kryo-radius flex items-center gap-3 border border-border px-3 py-2.5">
              <Avatar account={a} className="size-8" />
              <div className="grid grow">
                <span className="text-xs font-bold">{a.name}</span>
                <span className="text-[10px] uppercase tracking-wider text-muted-foreground">{a.kind === 'microsoft' ? 'Microsoft · owns Minecraft' : 'Offline profile'}</span>
              </div>
              {accounts.active === a.id ? (
                <Caption className="text-foreground">Playing as</Caption>
              ) : (
                <Button size="sm" onClick={() => void api.setActiveAccount(a.id).then(live.refreshAccounts).catch(toastError)}>
                  Use
                </Button>
              )}
              <Button size="sm" variant="ghost" aria-label={`Remove ${a.name}`} onClick={() => void api.removeAccount(a.id).then(live.refreshAccounts).catch(toastError)}>
                <Trash2 className="size-3" />
              </Button>
            </li>
          ))}
        </ul>
        <div className="flex flex-wrap gap-2">
          <Button variant="primary" onClick={() => onAccount('microsoft')}>
            <LogIn className="size-3.5" /> Sign in with Microsoft
          </Button>
          <Button onClick={() => onAccount('offline')}>
            <UserRound className="size-3.5" /> Add an offline profile
          </Button>
        </div>
      </Section>
      <Section title="What each kind can do">
        <dl className="grid gap-2 text-[11px] leading-relaxed text-muted-foreground">
          <div>
            <dt className="font-bold text-foreground/90">Microsoft account</dt>
            <dd>Any server, Realms, your skin. Must own Minecraft: Java Edition. You sign in on Microsoft's page; the launcher keeps a sign-in token in Windows' credential store, never your password.</dd>
          </div>
          <div>
            <dt className="font-bold text-foreground/90">Offline profile</dt>
            <dd>Singleplayer and offline-mode servers, under any name. Servers that check accounts turn it away.</dd>
          </div>
        </dl>
      </Section>
    </>
  )
}

function LookPane() {
  const { settings, saveSettings } = useApp()
  return (
    <>
      <Section title="Palette" hint="kryo.to's palettes: pick the same one as on the site.">
        <div className="flex flex-wrap gap-1.5">
          {PALETTES.map((p) => (
            <Button key={p.value} size="sm" variant={settings.palette === p.value ? 'primary' : 'outline'} onClick={() => void saveSettings({ palette: p.value })}>
              {p.label}
            </Button>
          ))}
        </div>
      </Section>
      <Section title="Corners">
        <Segmented value={settings.radius} onChange={(v) => void saveSettings({ radius: v })} options={RADII} />
      </Section>
      <Section title="Type">
        <Segmented value={settings.font} onChange={(v) => void saveSettings({ font: v })} options={FONTS} />
      </Section>
    </>
  )
}

function AboutPane() {
  const { info } = useApp()
  return (
    <>
      <section className="grid justify-items-center gap-4 py-4 text-center">
        <KryoMark className="h-8" />
        <div className="grid gap-1">
          <h2 className="text-sm font-bold uppercase tracking-[0.25em]">Kryoto Minecraft Launcher</h2>
          <Caption>Version {info.version} · {info.platform}</Caption>
        </div>
        <UpdateCheck />
      </section>
      <Section title="Data folder" hint="Versions, Java, libraries and every instance live here.">
        <div className="flex items-center gap-2">
          <code className="kryo-ascii-art kryo-radius min-w-0 grow truncate border border-border bg-background px-3 py-2 text-[11px]">{info.dataDir}</code>
          <Button onClick={() => void api.openDataFolder().catch(toastError)}>
            <FolderOpen className="size-3.5" /> Open
          </Button>
          <Button onClick={() => void api.openDataFolder('logs').catch(toastError)}>Logs</Button>
        </div>
      </Section>
      <Section title="Where things come from">
        <p className="text-[11px] leading-relaxed text-muted-foreground">
          Minecraft and its Java runtimes download from Mojang's own servers; this installer contains none of Mojang's files. Fabric, Quilt, Forge and NeoForge come from their makers, mods and packs from Modrinth.
        </p>
        <p className="text-[11px] leading-relaxed text-muted-foreground">Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.</p>
        <div className="flex flex-wrap gap-2">
          <Button size="sm" onClick={() => void openExternal('https://www.minecraft.net/en-us/store/minecraft-java-bedrock-edition-pc')}>
            Buy Minecraft <ArrowUpRight className="size-3" />
          </Button>
          <Button size="sm" onClick={() => void openExternal('https://kryo.to')}>
            kryo.to <ArrowUpRight className="size-3" />
          </Button>
        </div>
      </Section>
    </>
  )
}
