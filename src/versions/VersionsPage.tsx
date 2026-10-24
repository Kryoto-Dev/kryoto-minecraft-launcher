import { useEffect, useMemo, useState } from 'react'
import { Check as CheckIcon, Coffee, Info, Plus, RotateCw, Search, X } from 'lucide-react'
import { Busy, Button, Caption, IconButton, Label, Segmented } from '@/ui'
import { api, KIND_LABEL, LOADERS, type LoaderKind, type VersionDetails, type VersionKind, type VersionList, type VersionRow } from '@/lib/api'
import { errorText } from '@/lib/bridge'
import { bytes, date } from '@/lib/units'
import { cn } from '@/lib/utils'
import { versionNotes } from './versionNotes'

type Filter = 'release' | 'snapshot' | 'old'

/** Every Java Edition version Mojang lists, newest first. */
export function VersionsPage({ versions, error, onRefresh, onCreate }: { versions: VersionList | null; error: string | null; onRefresh: () => void; onCreate: (v: VersionRow) => void }) {
  const [filter, setFilter] = useState<Filter>('release')
  const [query, setQuery] = useState('')
  const [selected, setSelected] = useState<string | null>(null)

  const shown = useMemo(() => {
    if (!versions) return []
    const q = query.trim().toLowerCase()
    return versions.versions.filter((v) => {
      if (q) return v.id.toLowerCase().includes(q)
      if (filter === 'old') return v.kind === 'old_beta' || v.kind === 'old_alpha'
      return v.kind === filter
    })
  }, [versions, filter, query])

  useEffect(() => {
    if (!selected && shown[0]) setSelected(shown[0].id)
  }, [shown, selected])

  const current = versions?.versions.find((v) => v.id === selected) ?? null
  const counts = useMemo(() => {
    const c = { release: 0, snapshot: 0, old: 0 }
    for (const v of versions?.versions ?? []) {
      if (v.kind === 'release') c.release++
      else if (v.kind === 'snapshot') c.snapshot++
      else c.old++
    }
    return c
  }, [versions])

  if (error && !versions) {
    return (
      <div className="grid h-full place-items-center p-8 text-center">
        <div className="grid max-w-sm justify-items-center gap-3">
          <p className="text-xs text-destructive">{error}</p>
          <Button onClick={onRefresh}>
            <RotateCw className="size-3.5" /> Try again
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_380px]">
      <div className="flex min-h-0 flex-col border-r border-border">
        <div className="flex shrink-0 flex-wrap items-center gap-2 px-5 py-3">
          <Segmented
            value={filter}
            onChange={(f) => {
              setFilter(f)
              setQuery('')
            }}
            label="Kind of version"
            options={[
              { value: 'release', label: `Releases ${counts.release || ''}`.trim() },
              { value: 'snapshot', label: `Snapshots ${counts.snapshot || ''}`.trim() },
              { value: 'old', label: `Beta & Alpha ${counts.old || ''}`.trim() },
            ]}
          />
          <label className="kryo-pill ml-auto flex h-9 w-56 items-center gap-2 border border-border px-3 focus-within:border-foreground">
            <Search className="size-3.5 text-muted-foreground" />
            <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Find a version" className="min-w-0 grow bg-transparent text-xs outline-none placeholder:text-muted-foreground" />
            {query ? (
              <button type="button" aria-label="Clear" onClick={() => setQuery('')} className="kryo-square text-muted-foreground hover:text-foreground">
                <X className="size-3" />
              </button>
            ) : null}
          </label>
          <IconButton label="Refresh the list from Mojang" onClick={onRefresh}>
            <RotateCw className="size-3.5" />
          </IconButton>
        </div>
        <div role="listbox" aria-label="Versions" className="min-h-0 grow overflow-y-auto px-3 pb-4">
          {!versions ? (
            <div className="grid place-items-center py-16">
              <Busy className="size-5" />
            </div>
          ) : (
            shown.map((v) => (
              <button
                key={v.id}
                type="button"
                role="option"
                aria-selected={selected === v.id}
                onClick={() => setSelected(v.id)}
                onDoubleClick={() => onCreate(v)}
                className={cn(
                  'kryo-radius flex w-full items-center gap-3 px-3 py-2 text-left transition-colors',
                  selected === v.id ? 'bg-secondary' : 'hover:bg-secondary/50',
                )}
              >
                <span className="w-44 truncate text-xs font-bold">{v.id}</span>
                <Caption className="w-20">{KIND_LABEL[v.kind]}</Caption>
                <span className="grow text-[11px] text-muted-foreground">{date(v.releaseTime)}</span>
                {v.id === versions.latestRelease || v.id === versions.latestSnapshot ? (
                  <span className="kryo-pill border border-border px-2 py-0.5 text-[9px] uppercase tracking-wider text-foreground">Latest</span>
                ) : null}
                {v.installed ? <CheckIcon className="size-3.5 text-success" aria-label="Downloaded" /> : null}
              </button>
            ))
          )}
          {versions && shown.length === 0 ? <p className="px-3 py-6 text-xs text-muted-foreground">No version matches "{query}".</p> : null}
        </div>
      </div>
      <aside className="min-h-0 overflow-y-auto">{current ? <VersionPanel key={current.id} version={current} onCreate={() => onCreate(current)} /> : null}</aside>
    </div>
  )
}

function VersionPanel({ version, onCreate }: { version: VersionRow; onCreate: () => void }) {
  const [details, setDetails] = useState<VersionDetails | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loaders, setLoaders] = useState<Partial<Record<LoaderKind, boolean>>>({})
  useEffect(() => {
    let cancelled = false
    api
      .versionDetails(version.id)
      .then((d) => !cancelled && setDetails(d))
      .catch((e) => !cancelled && setError(errorText(e)))
    for (const l of LOADERS.filter((l) => l.kind !== 'vanilla')) {
      api
        .loaderVersions(l.kind, version.id)
        .then((list) => !cancelled && setLoaders((s) => ({ ...s, [l.kind]: list.length > 0 })))
        .catch(() => !cancelled && setLoaders((s) => ({ ...s, [l.kind]: false })))
    }
    return () => {
      cancelled = true
    }
  }, [version.id])
  const notes = versionNotes(version.id, version.kind as VersionKind)

  return (
    <div className="kryo-in grid gap-6 p-6">
      <div className="grid gap-2">
        <Caption>{KIND_LABEL[version.kind]} · {date(version.releaseTime)}</Caption>
        <h2 className="text-3xl font-bold">{version.id}</h2>
      </div>
      <Button variant="primary" size="lg" onClick={onCreate} className="justify-self-start">
        <Plus className="size-4" /> Create instance
      </Button>

      <section className="grid gap-2.5">
        <Label>Details</Label>
        {error ? (
          <p className="text-[11px] text-destructive">{error}</p>
        ) : !details ? (
          <Busy className="size-4" />
        ) : (
          <dl className="grid gap-1.5 text-[11px]">
            <Row k="Java" v={<span className="flex items-center gap-1.5"><Coffee className="size-3" /> Java {details.javaMajor}, downloaded for you</span>} />
            {details.clientSize ? <Row k="Game" v={bytes(details.clientSize)} /> : null}
            {details.assetsSize ? <Row k="Sounds and textures" v={`${bytes(details.assetsSize)}, shared between versions`} /> : null}
            <Row k="Libraries" v={String(details.libraryCount)} />
            <Row k="Downloaded" v={version.installed ? 'Yes' : 'Not yet'} />
          </dl>
        )}
      </section>

      <section className="grid gap-2.5">
        <Label>Mod loaders</Label>
        <div className="flex flex-wrap gap-1.5">
          {LOADERS.filter((l) => l.kind !== 'vanilla').map((l) => {
            const has = loaders[l.kind]
            return (
              <span key={l.kind} className={cn('kryo-pill flex h-7 items-center gap-1.5 border px-3 text-[10px] uppercase tracking-wider', has ? 'border-foreground/60 text-foreground' : 'border-border text-muted-foreground/60')}>
                {has === undefined ? <Busy className="size-2.5" /> : has ? <CheckIcon className="size-3" /> : <X className="size-3" />}
                {l.label}
              </span>
            )
          })}
        </div>
      </section>

      {notes.length ? (
        <section className="grid gap-2">
          <Label>Good to know</Label>
          {notes.map((n) => (
            <p key={n} className="flex gap-2 text-[11px] leading-relaxed text-muted-foreground">
              <Info className="mt-0.5 size-3 shrink-0" /> {n}
            </p>
          ))}
        </section>
      ) : null}
    </div>
  )
}

function Row({ k, v }: { k: string; v: React.ReactNode }) {
  return (
    <div className="flex justify-between gap-4">
      <dt className="uppercase tracking-wider text-muted-foreground">{k}</dt>
      <dd className="text-right text-foreground/90">{v}</dd>
    </div>
  )
}
