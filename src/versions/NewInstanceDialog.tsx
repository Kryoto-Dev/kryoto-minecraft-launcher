import { useEffect, useMemo, useState } from 'react'
import { Button, Caption, Dropdown, Modal, inputCls } from '@/ui'
import { BLOCK_NAMES, BlockIcon } from '@/ui/BlockIcon'
import { toastError } from '@/ui/Toasts'
import { api, KIND_LABEL, LOADERS, type Instance, type LoaderKind, type LoaderVersion, type VersionList } from '@/lib/api'
import { errorText } from '@/lib/bridge'
import { live } from '@/lib/store'
import { cn } from '@/lib/utils'

/**
 * A new instance: a version, a loader if wanted, a name. Creating it starts
 * the download at once; Play waits for it.
 */
export function NewInstanceDialog({ versions, initial, onClose, onCreated }: { versions: VersionList; initial?: string; onClose: () => void; onCreated: (i: Instance) => void }) {
  const [game, setGame] = useState(initial ?? versions.latestRelease)
  const [showAll, setShowAll] = useState(() => !!initial && versions.versions.find((v) => v.id === initial)?.kind !== 'release')
  const [loader, setLoader] = useState<LoaderKind>('vanilla')
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersion[] | null>(null)
  const [loaderVersion, setLoaderVersion] = useState<string | null>(null)
  const [loaderError, setLoaderError] = useState<string | null>(null)
  const [icon, setIcon] = useState(() => BLOCK_NAMES[Math.floor(Math.random() * BLOCK_NAMES.length)] ?? 'grass')
  const [name, setName] = useState('')
  const [nameTouched, setNameTouched] = useState(false)
  const [creating, setCreating] = useState(false)

  const row = versions.versions.find((v) => v.id === game)
  const options = useMemo(
    () =>
      versions.versions
        .filter((v) => showAll || v.kind === 'release' || v.id === game)
        .map((v) => ({ value: v.id, label: v.id, hint: v.kind === 'release' ? undefined : KIND_LABEL[v.kind] })),
    [versions, showAll, game],
  )

  useEffect(() => {
    if (!nameTouched) setName(loader === 'vanilla' ? `Minecraft ${game}` : `${LOADERS.find((l) => l.kind === loader)?.label} ${game}`)
  }, [game, loader, nameTouched])

  useEffect(() => {
    setLoaderVersions(null)
    setLoaderVersion(null)
    setLoaderError(null)
    if (loader === 'vanilla') return
    let cancelled = false
    api
      .loaderVersions(loader, game)
      .then((list) => {
        if (cancelled) return
        setLoaderVersions(list)
        setLoaderVersion(list.find((v) => v.recommended)?.version ?? list[0]?.version ?? null)
      })
      .catch((e) => !cancelled && setLoaderError(errorText(e)))
    return () => {
      cancelled = true
    }
  }, [loader, game])

  const noLoader = loader !== 'vanilla' && loaderVersions !== null && loaderVersions.length === 0
  const ready = !!row && name.trim().length > 0 && !noLoader && (loader === 'vanilla' || !!loaderVersion)

  const create = async () => {
    if (!row) return
    setCreating(true)
    try {
      const created = await api.createInstance({ name: name.trim(), gameVersion: row.id, versionType: row.kind, loader: { kind: loader, version: loader === 'vanilla' ? null : loaderVersion }, icon })
      await live.refreshInstances()
      onCreated(created)
    } catch (e) {
      toastError(e)
      setCreating(false)
    }
  }

  return (
    <Modal
      title="New instance"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!ready || creating} onClick={() => void create()}>
            Create and download
          </Button>
        </>
      }
    >
      <div className="grid gap-5">
        <div className="grid gap-2">
          <Caption>Minecraft version</Caption>
          <div className="flex items-center gap-3">
            <Dropdown className="grow" value={game} options={options} onChange={setGame} label="Minecraft version" />
            <button type="button" onClick={() => setShowAll((s) => !s)} className="kryo-square shrink-0 text-[10px] uppercase tracking-wider text-muted-foreground hover:text-foreground">
              {showAll ? 'Releases only' : 'Show snapshots and old'}
            </button>
          </div>
        </div>

        <div className="grid gap-2">
          <Caption>Mod loader</Caption>
          <div className="flex flex-wrap gap-1.5">
            {LOADERS.map((l) => (
              <Button key={l.kind} size="sm" variant={loader === l.kind ? 'primary' : 'outline'} onClick={() => setLoader(l.kind)}>
                {l.label}
              </Button>
            ))}
          </div>
          {loader === 'vanilla' ? (
            <p className="text-[11px] text-muted-foreground">{LOADERS[0]!.about}</p>
          ) : loaderError ? (
            <p className="text-[11px] text-destructive">{loaderError}</p>
          ) : noLoader ? (
            <p className="text-[11px] text-muted-foreground">There's no {LOADERS.find((l) => l.kind === loader)?.label} for Minecraft {game} that the launcher can install. Pick another loader or version.</p>
          ) : loaderVersions ? (
            <div className="flex items-center gap-2">
              <Dropdown
                className="w-64"
                size="sm"
                value={loaderVersion ?? ''}
                onChange={setLoaderVersion}
                label="Loader version"
                options={loaderVersions.slice(0, 60).map((v) => ({ value: v.version, label: v.label, hint: v.recommended ? 'Recommended' : v.stable ? undefined : 'Beta' }))}
              />
              <span className="text-[11px] text-muted-foreground">{LOADERS.find((l) => l.kind === loader)?.about}</span>
            </div>
          ) : (
            <Caption>Looking up versions</Caption>
          )}
        </div>

        <label className="grid gap-2">
          <Caption>Name</Caption>
          <input
            value={name}
            maxLength={64}
            onChange={(e) => {
              setName(e.target.value)
              setNameTouched(true)
            }}
            className={inputCls}
          />
        </label>

        <div className="grid gap-2">
          <Caption>Icon</Caption>
          <div className="flex flex-wrap gap-1.5">
            {BLOCK_NAMES.map((b) => (
              <button
                key={b}
                type="button"
                aria-pressed={icon === b}
                aria-label={b}
                onClick={() => setIcon(b)}
                className={cn('kryo-radius grid size-10 place-items-center border transition-colors', icon === b ? 'border-foreground bg-secondary' : 'border-border hover:border-foreground/60')}
              >
                <BlockIcon name={b} className="size-6" />
              </button>
            ))}
          </div>
        </div>
      </div>
    </Modal>
  )
}
