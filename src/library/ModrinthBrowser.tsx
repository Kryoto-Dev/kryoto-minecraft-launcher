import { useEffect, useRef, useState } from 'react'
import { ArrowUpRight, Check as CheckIcon, Download, Search } from 'lucide-react'
import { Busy, Button, Dropdown, Modal } from '@/ui'
import { toast, toastError } from '@/ui/Toasts'
import { api, loaderLabel, type ContentItem, type ContentKind, type Instance, type ModrinthHit } from '@/lib/api'
import { errorText } from '@/lib/bridge'
import { count } from '@/lib/units'
import { openExternal } from '@/lib/window'

const SORTS = [
  { value: 'relevance', label: 'Relevance' },
  { value: 'downloads', label: 'Downloads' },
  { value: 'follows', label: 'Follows' },
  { value: 'updated', label: 'Updated' },
  { value: 'newest', label: 'Newest' },
]

const TITLES: Record<ContentKind, string> = { mod: 'mods', resourcepack: 'resource packs', shaderpack: 'shader packs', world: 'worlds' }

/**
 * Modrinth, filtered to what runs on this instance: its Minecraft version and,
 * for mods, its loader. Adding a mod also adds the mods it requires.
 */
export function ModrinthBrowser({ instance, kind, installed, onClose }: { instance: Instance; kind: ContentKind; installed: ContentItem[]; onClose: () => void }) {
  const [query, setQuery] = useState('')
  const [sort, setSort] = useState('relevance')
  const [hits, setHits] = useState<ModrinthHit[] | null>(null)
  const [total, setTotal] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [adding, setAdding] = useState<Set<string>>(new Set())
  const [added, setAdded] = useState<Set<string>>(() => new Set(installed.map((i) => i.source?.projectId).filter((x): x is string => !!x)))
  const seq = useRef(0)

  const search = async (offset: number) => {
    const n = ++seq.current
    if (offset === 0) setHits(null)
    try {
      const r = await api.modrinthSearch(instance.id, kind, query, sort, offset)
      if (n !== seq.current) return
      setHits((prev) => (offset === 0 ? r.hits : [...(prev ?? []), ...r.hits]))
      setTotal(r.total_hits)
      setError(null)
    } catch (e) {
      if (n === seq.current) setError(errorText(e))
    }
  }
  useEffect(() => {
    const t = window.setTimeout(() => void search(0), query ? 300 : 0)
    return () => window.clearTimeout(t)
  }, [query, sort])

  const add = async (hit: ModrinthHit) => {
    setAdding((s) => new Set(s).add(hit.project_id))
    try {
      const titles = await api.modrinthInstall(instance.id, kind, hit.project_id, hit.title)
      setAdded((s) => new Set(s).add(hit.project_id))
      toast(titles.length > 1 ? `Added ${titles[0]} and what it needs: ${titles.slice(1).join(', ')}.` : `Added ${hit.title}.`)
    } catch (e) {
      toastError(e)
    } finally {
      setAdding((s) => {
        const n = new Set(s)
        n.delete(hit.project_id)
        return n
      })
    }
  }

  const target = kind === 'mod' ? `${loaderLabel(instance.loader.kind)} ${instance.gameVersion}` : `Minecraft ${instance.gameVersion}`

  return (
    <Modal title={`Modrinth · ${TITLES[kind]} for ${target}`} onClose={onClose} wide fill>
      <div className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
        <div className="flex items-center gap-2">
          <label className="kryo-pill flex h-9 grow items-center gap-2 border border-border px-3 focus-within:border-foreground">
            <Search className="size-3.5 text-muted-foreground" />
            <input autoFocus value={query} onChange={(e) => setQuery(e.target.value)} placeholder={`Search ${TITLES[kind]}`} className="min-w-0 grow bg-transparent text-xs outline-none placeholder:text-muted-foreground" />
          </label>
          <Dropdown value={sort} options={SORTS} onChange={setSort} className="w-40" label="Sort by" />
        </div>
        <div className="-mx-1 min-h-0 overflow-y-auto px-1">
          {error ? (
            <p role="alert" className="py-6 text-xs text-destructive">
              {error}
            </p>
          ) : !hits ? (
            <div className="grid place-items-center py-16">
              <Busy className="size-5" />
            </div>
          ) : hits.length === 0 ? (
            <p className="py-10 text-center text-xs text-muted-foreground">Nothing on Modrinth matches for {target}.</p>
          ) : (
            <ul className="grid gap-1">
              {hits.map((hit) => {
                const busy = adding.has(hit.project_id)
                const done = added.has(hit.project_id)
                return (
                  <li key={hit.project_id} className="kryo-radius flex items-center gap-3 border border-transparent px-3 py-2.5 transition-colors hover:border-border hover:bg-background/60">
                    {hit.icon_url ? (
                      <img src={hit.icon_url} alt="" loading="lazy" className="kryo-square size-11 shrink-0 rounded-[6px] object-cover" />
                    ) : (
                      <span className="kryo-radius size-11 shrink-0 bg-secondary" />
                    )}
                    <div className="grid min-w-0 grow gap-0.5">
                      <span className="flex items-baseline gap-2">
                        <button type="button" onClick={() => void openExternal(`https://modrinth.com/project/${hit.slug}`)} className="kryo-square truncate text-left text-xs font-bold hover:underline">
                          {hit.title}
                        </button>
                        <span className="shrink-0 text-[10px] text-muted-foreground">by {hit.author}</span>
                      </span>
                      <span className="line-clamp-2 text-[11px] leading-relaxed text-muted-foreground">{hit.description}</span>
                    </div>
                    <span className="hidden shrink-0 items-center gap-1 text-[10px] tabular-nums text-muted-foreground md:flex">
                      <Download className="size-3" /> {count(hit.downloads)}
                    </span>
                    <Button size="sm" variant={done ? 'ghost' : 'outline'} disabled={busy || done} onClick={() => void add(hit)} className="w-24">
                      {busy ? <Busy className="size-3" /> : done ? <CheckIcon className="size-3" /> : null}
                      {busy ? 'Adding' : done ? 'Added' : 'Add'}
                    </Button>
                  </li>
                )
              })}
            </ul>
          )}
          {hits && hits.length < total ? (
            <div className="flex justify-center py-4">
              <Button size="sm" onClick={() => void search(hits.length)}>
                More results
              </Button>
            </div>
          ) : null}
          <p className="flex items-center gap-1 pb-2 pt-4 text-[10px] text-muted-foreground">
            Results from Modrinth.
            <button type="button" className="kryo-square inline-flex items-center gap-0.5 underline" onClick={() => void openExternal('https://modrinth.com')}>
              modrinth.com <ArrowUpRight className="size-3" />
            </button>
          </p>
        </div>
      </div>
    </Modal>
  )
}
