import { useEffect, useState } from 'react'
import { Trash2, Wrench } from 'lucide-react'
import { Button, Caption, Check, Dropdown, Modal, Section, inputCls } from '@/ui'
import { BLOCKS, BLOCK_NAMES, BlockIcon, blockFor } from '@/ui/BlockIcon'
import { JavaPicker, MemorySlider, NumberField } from '@/ui/Controls'
import { toast, toastError } from '@/ui/Toasts'
import { api, LOADERS, type Instance, type LoaderKind, type LoaderVersion, type Overrides } from '@/lib/api'
import { useApp } from '@/lib/app-context'
import { errorText } from '@/lib/bridge'
import { live, useGames } from '@/lib/store'
import { cn } from '@/lib/utils'

export function InstanceSettings({ instance, onDeleted }: { instance: Instance; onDeleted: () => void }) {
  const { info, settings } = useApp()
  const games = useGames()
  const running = !!games[instance.id]?.running
  const [name, setName] = useState(instance.name)
  const [confirmDelete, setConfirmDelete] = useState(false)
  useEffect(() => setName(instance.name), [instance.name])
  const o = instance.overrides

  const save = async (patch: Parameters<typeof api.updateInstance>[1]) => {
    try {
      await api.updateInstance(instance.id, patch)
      await live.refreshInstances()
    } catch (e) {
      toastError(e)
    }
  }
  const override = (patch: Partial<Overrides>) => save({ overrides: { ...o, ...patch } })

  return (
    <div className="h-full overflow-y-auto">
      <div className="grid max-w-3xl gap-8 px-6 py-5">
        <Section title="Name and icon">
          <input value={name} maxLength={64} onChange={(e) => setName(e.target.value)} onBlur={() => name.trim() && name !== instance.name && void save({ name })} className={cn(inputCls, 'max-w-sm')} />
          <div className="flex flex-wrap gap-1.5">
            {BLOCK_NAMES.map((b) => {
              const current = (instance.icon ?? blockFor(instance.id)) === b
              return (
                <button
                  key={b}
                  type="button"
                  title={BLOCKS[b]!.label}
                  aria-pressed={current}
                  onClick={() => void save({ icon: b })}
                  className={cn('kryo-radius grid size-11 place-items-center border transition-colors', current ? 'border-foreground bg-secondary' : 'border-border hover:border-foreground/60')}
                >
                  <BlockIcon name={b} className="size-7" />
                </button>
              )
            })}
          </div>
        </Section>

        <LoaderSection instance={instance} running={running} />

        <Section title="Memory" hint="How much memory Minecraft may use.">
          <Check checked={o.memoryMb === null} onChange={(on) => void override({ memoryMb: on ? null : settings.memoryMb })} label={`Use the launcher's setting (${(settings.memoryMb / 1024).toFixed(1)} GB)`} />
          {o.memoryMb !== null ? <MemorySlider value={o.memoryMb} totalMb={info.totalMemoryMb} onChange={(mb) => void override({ memoryMb: mb })} /> : null}
        </Section>

        <Section title="Java">
          <JavaPicker value={o.javaPath} onChange={(p) => void override({ javaPath: p })} />
        </Section>

        <Section title="Window">
          <Check checked={o.width === null} onChange={(on) => void override(on ? { width: null, height: null, fullscreen: null } : { width: settings.width, height: settings.height, fullscreen: settings.fullscreen })} label="Use the launcher's setting" />
          {o.width !== null ? (
            <div className="flex flex-wrap items-center gap-3">
              <NumberField label="Width" min={320} value={o.width ?? settings.width} onChange={(w) => void override({ width: w })} />
              <span className="text-muted-foreground">×</span>
              <NumberField label="Height" min={240} value={o.height ?? settings.height} onChange={(h) => void override({ height: h })} />
              <Check checked={o.fullscreen ?? false} onChange={(f) => void override({ fullscreen: f })} label="Full screen" />
            </div>
          ) : null}
        </Section>

        <Section title="Java arguments" hint="For players who know what they need. Memory is set above; leave this empty otherwise. A garbage collector you pick (-XX:+UseZGC) replaces the launcher's own.">
          <input
            defaultValue={o.jvmArgs ?? ''}
            placeholder={settings.jvmArgs || '-XX:+UseZGC'}
            spellCheck={false}
            onBlur={(e) => {
              const v = e.target.value.trim()
              if (v !== (o.jvmArgs ?? '')) void override({ jvmArgs: v || null })
            }}
            className={inputCls}
          />
        </Section>

        <Section title="Files" hint="Repair checks every game file against Mojang's checksums and downloads what is damaged or missing. Worlds and mods are not touched.">
          <div className="flex flex-wrap gap-2">
            <Button disabled={running} onClick={() => void api.install(instance.id, true).then(() => toast(`${instance.name} checked and repaired.`)).catch(toastError)}>
              <Wrench className="size-3.5" /> Repair
            </Button>
            <Button variant="danger" disabled={running} onClick={() => setConfirmDelete(true)}>
              <Trash2 className="size-3.5" /> Delete instance
            </Button>
          </div>
        </Section>
      </div>

      {confirmDelete ? (
        <Modal
          title="Delete instance"
          onClose={() => setConfirmDelete(false)}
          footer={
            <>
              <Button onClick={() => setConfirmDelete(false)}>Keep it</Button>
              <Button
                variant="danger"
                onClick={() =>
                  void api
                    .deleteInstance(instance.id)
                    .then(() => live.refreshInstances())
                    .then(() => {
                      toast(`${instance.name} deleted.`)
                      onDeleted()
                    })
                    .catch(toastError)
                }
              >
                Delete for good
              </Button>
            </>
          }
        >
          <p className="text-xs leading-relaxed">
            Delete <b>{instance.name}</b> and its game folder? Its <b>worlds</b>, mods, packs and settings go with it, and this can't be undone. Minecraft's own files stay for other instances.
          </p>
        </Modal>
      ) : null}
    </div>
  )
}

/** Change the loader or its version; the instance installs again on Play. */
function LoaderSection({ instance, running }: { instance: Instance; running: boolean }) {
  const [kind, setKind] = useState<LoaderKind>(instance.loader.kind)
  const [versions, setVersions] = useState<LoaderVersion[] | null>(null)
  const [version, setVersion] = useState<string | null>(instance.loader.version)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    setKind(instance.loader.kind)
    setVersion(instance.loader.version)
  }, [instance.loader.kind, instance.loader.version])
  useEffect(() => {
    setVersions(null)
    setError(null)
    if (kind === 'vanilla') return
    let cancelled = false
    api
      .loaderVersions(kind, instance.gameVersion)
      .then((v) => {
        if (cancelled) return
        setVersions(v)
        if (kind !== instance.loader.kind || !version) setVersion(v.find((x) => x.recommended)?.version ?? v[0]?.version ?? null)
      })
      .catch((e) => !cancelled && setError(errorText(e)))
    return () => {
      cancelled = true
    }
  }, [kind, instance.gameVersion])

  const changed = kind !== instance.loader.kind || (kind !== 'vanilla' && version !== instance.loader.version)
  const unavailable = kind !== 'vanilla' && versions !== null && versions.length === 0
  const apply = async () => {
    try {
      await api.updateInstance(instance.id, { loader: { kind, version: kind === 'vanilla' ? null : version } })
      await live.refreshInstances()
      toast(kind === 'vanilla' ? 'Back to vanilla. Mods stay in the mods folder, unused.' : 'Loader changed. It installs when you press Play.')
    } catch (e) {
      toastError(e)
    }
  }

  return (
    <Section title="Mod loader" hint="Mods need a loader, and only run on the loader they were made for.">
      <div className="flex flex-wrap gap-1.5">
        {LOADERS.map((l) => (
          <Button key={l.kind} size="sm" variant={kind === l.kind ? 'primary' : 'outline'} onClick={() => setKind(l.kind)}>
            {l.label}
          </Button>
        ))}
      </div>
      {kind !== 'vanilla' ? (
        error ? (
          <p className="text-[11px] text-destructive">{error}</p>
        ) : unavailable ? (
          <p className="text-[11px] text-muted-foreground">There's no {LOADERS.find((l) => l.kind === kind)?.label} for Minecraft {instance.gameVersion} that the launcher can install.</p>
        ) : versions ? (
          <div className="flex items-center gap-2">
            <Caption>Version</Caption>
            <Dropdown
              className="w-64"
              value={version ?? ''}
              onChange={setVersion}
              label="Loader version"
              options={versions.slice(0, 60).map((v) => ({ value: v.version, label: v.label, hint: v.recommended ? 'Recommended' : v.stable ? undefined : 'Beta' }))}
            />
          </div>
        ) : (
          <Caption>Looking up versions</Caption>
        )
      ) : null}
      {changed && !unavailable ? (
        <Button size="sm" variant="primary" className="justify-self-start" disabled={running || (kind !== 'vanilla' && !version)} onClick={() => void apply()}>
          Use {LOADERS.find((l) => l.kind === kind)?.label}
          {kind !== 'vanilla' && version ? ` ${versions?.find((v) => v.version === version)?.label ?? version}` : ''}
        </Button>
      ) : null}
    </Section>
  )
}
