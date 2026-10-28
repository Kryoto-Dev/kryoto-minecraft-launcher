import { useState } from 'react'
import { ArrowUpRight, FolderOpen, Trash2, UserRound } from 'lucide-react'
import { Button, Caption, Check, Dropdown, Section, Segmented, inputCls } from '@/ui'
import { KryoMark } from '@/ui/ascii/KryoMark'
import { JavaPicker, MemorySlider, NumberField } from '@/ui/Controls'
import { toastError } from '@/ui/Toasts'
import { api } from '@/lib/api'
import { useApp } from '@/lib/app-context'
import { FONTS, PALETTES, RADII } from '@/lib/look'
import { live, useProfiles } from '@/lib/store'
import { openExternal } from '@/lib/window'
import { cn } from '@/lib/utils'
import { Avatar } from '@/profiles/Profiles'
import { UpdateCheck } from '@/shell/UpdatePrompt'

type Pane = 'game' | 'profiles' | 'look' | 'about'

const PANES: [Pane, string][] = [
  ['game', 'Game'],
  ['profiles', 'Profiles'],
  ['look', 'Interface'],
  ['about', 'About'],
]

export function SettingsPage({ onAddProfile }: { onAddProfile: () => void }) {
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
          {pane === 'game' ? <GamePane /> : pane === 'profiles' ? <ProfilesPane onAddProfile={onAddProfile} /> : pane === 'look' ? <LookPane /> : <AboutPane />}
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
      <Section title="Java arguments" hint="Added to every start. Leave empty unless you know you need something. A garbage collector you pick (-XX:+UseZGC) replaces the launcher's own.">
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

function ProfilesPane({ onAddProfile }: { onAddProfile: () => void }) {
  const profiles = useProfiles()
  return (
    <>
      <Section title="Profiles" hint="The name the game starts with is the one marked. Switch any time from the title bar.">
        {profiles.profiles.length === 0 ? <p className="text-xs text-muted-foreground">No profiles yet.</p> : null}
        <ul className="grid gap-1.5">
          {profiles.profiles.map((p) => (
            <li key={p.id} className="kryo-radius flex items-center gap-3 border border-border px-3 py-2.5">
              <Avatar profile={p} className="size-8" />
              <span className="grow text-xs font-bold">{p.name}</span>
              {profiles.active === p.id ? (
                <Caption className="text-foreground">Playing as</Caption>
              ) : (
                <Button size="sm" onClick={() => void api.setActiveProfile(p.id).then(live.refreshProfiles).catch(toastError)}>
                  Use
                </Button>
              )}
              <Button size="sm" variant="ghost" aria-label={`Remove ${p.name}`} onClick={() => void api.removeProfile(p.id).then(live.refreshProfiles).catch(toastError)}>
                <Trash2 className="size-3" />
              </Button>
            </li>
          ))}
        </ul>
        <Button variant="primary" className="justify-self-start" onClick={onAddProfile}>
          <UserRound className="size-3.5" /> New profile
        </Button>
      </Section>
      <Section title="What a profile can do">
        <p className="text-[11px] leading-relaxed text-muted-foreground">
          Singleplayer, LAN, and servers that run in offline mode, like play.kryo.to, under any name. Your worlds and a server's inventory follow the name. Servers that check
          Microsoft accounts, Realms and skins aren't available.
        </p>
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
      <Section
        title="Baby mobs"
        hint="Baby animals walk along the bottom of the window once a version with baby mobs (26.1 or newer) is installed. Their shapes and looks come from that game, a different mix every start. Click one."
      >
        <Check checked={settings.mobs} onChange={(on) => void saveSettings({ mobs: on })} label="Show baby mobs" />
        {settings.mobs ? (
          <>
            <div className="flex items-center gap-3">
              <Caption>At most</Caption>
              <Dropdown
                className="w-40"
                size="sm"
                label="How many baby mobs at most"
                value={String(settings.mobCount)}
                onChange={(v) => void saveSettings({ mobCount: Number(v) })}
                options={['1', '2', '3', '5', '8', '12'].map((v) => ({ value: v, label: v === '1' ? '1 mob' : `${v} mobs` }))}
              />
            </div>
            <Check
              checked={settings.friendlyOnly}
              onChange={(on) => void saveSettings({ friendlyOnly: on })}
              label="No mean monsters (baby zombies, drowned, piglins, hoglins)"
            />
          </>
        ) : null}
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
