import { useMemo, useState } from 'react'
import { Home, Plus, Search } from 'lucide-react'
import { AsciiBar, Caption } from '@/ui'
import { BlockIcon, blockFor } from '@/ui/BlockIcon'
import { instanceLabel, type Instance } from '@/lib/api'
import { taskFor, taskFraction, useGames, useTasks } from '@/lib/store'
import { cn } from '@/lib/utils'

/** The instance list, as Kryoto Desktop's library sidebar. */
export function Sidebar({ instances, selected, onSelect, onNew }: { instances: Instance[]; selected: string | null; onSelect: (id: string | null) => void; onNew: () => void }) {
  const [query, setQuery] = useState('')
  const tasks = useTasks()
  const games = useGames()
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase()
    const list = q ? instances.filter((i) => i.name.toLowerCase().includes(q) || instanceLabel(i).toLowerCase().includes(q)) : instances
    return [...list].sort((a, b) => a.name.localeCompare(b.name))
  }, [instances, query])
  const installing = instances.filter((i) => taskFor(tasks, i.id))

  return (
    <aside className="flex w-64 shrink-0 flex-col gap-3 border-r border-border bg-background p-2.5">
      <button
        type="button"
        onClick={() => onSelect(null)}
        aria-current={selected === null}
        className={cn(
          'kryo-pill flex h-9 items-center gap-2.5 px-4 text-[11px] font-bold uppercase tracking-[0.2em] transition-colors',
          selected === null ? 'bg-primary text-primary-foreground' : 'border border-border text-muted-foreground hover:border-foreground hover:text-foreground',
        )}
      >
        <Home className="size-3.5" /> Home
      </button>
      <label className="kryo-pill flex h-9 items-center gap-2 border border-border px-3 text-xs focus-within:border-foreground">
        <Search className="size-3.5 text-muted-foreground" />
        <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search instances" className="min-w-0 grow bg-transparent text-xs outline-none placeholder:text-muted-foreground" />
      </label>

      <div className="flex items-center justify-between px-1 pt-1">
        <span className="text-[10px] font-bold uppercase tracking-[0.25em] text-foreground/80">Instances</span>
        <span className="text-[10px] tabular-nums text-muted-foreground">{instances.length}</span>
      </div>
      <nav aria-label="Instances" className="-mx-1 grid min-h-0 content-start gap-0.5 overflow-y-auto px-1">
        {shown.map((i) => {
          const running = games[i.id]?.running
          return (
            <button
              key={i.id}
              type="button"
              onClick={() => onSelect(i.id)}
              aria-current={selected === i.id}
              className={cn(
                'kryo-radius flex items-center gap-2.5 px-2 py-1.5 text-left transition-colors',
                selected === i.id ? 'bg-secondary text-foreground' : 'text-foreground/85 hover:bg-secondary/60',
              )}
            >
              <BlockIcon name={i.icon ?? blockFor(i.id)} className="size-6" glint={!!running} progress={!i.installed && taskFor(tasks, i.id) ? taskFraction(taskFor(tasks, i.id)!) : null} />
              <span className="grid min-w-0 grow">
                <span className="truncate text-xs">{i.name}</span>
                <span className="truncate text-[10px] text-muted-foreground">{instanceLabel(i)}</span>
              </span>
              {running ? <span title="Running" className="size-1.5 shrink-0 rounded-full bg-success" /> : !i.installed ? <Caption className="text-[9px]">New</Caption> : null}
            </button>
          )
        })}
        {shown.length === 0 && instances.length > 0 ? <p className="px-2 py-1 text-[11px] text-muted-foreground">No instance matches.</p> : null}
      </nav>

      {installing.length ? (
        <div className="grid gap-2 border-t border-border px-1 pt-3">
          <span className="text-[10px] font-bold uppercase tracking-[0.25em] text-foreground/80">Working</span>
          {installing.map((i) => {
            const t = taskFor(tasks, i.id)!
            const f = taskFraction(t)
            return (
              <button key={i.id} type="button" onClick={() => onSelect(i.id)} className="kryo-square grid gap-1 text-left">
                <span className="truncate text-[11px] text-foreground/85">{i.name}</span>
                <span className="flex items-center justify-between gap-2">
                  <AsciiBar fraction={f} cells={14} showPct={false} className="text-[10px]" />
                  <span className="truncate text-[10px] text-muted-foreground">{t.stage}</span>
                </span>
              </button>
            )
          })}
        </div>
      ) : null}

      <button type="button" onClick={onNew} className="kryo-pill mt-auto flex h-9 items-center justify-center gap-2 border border-dashed border-border text-[11px] uppercase tracking-wider text-muted-foreground transition-colors hover:border-foreground hover:text-foreground">
        <Plus className="size-3.5" /> New instance
      </button>
    </aside>
  )
}
