import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { AlertTriangle, Copy, FileWarning, FolderOpen, Search } from 'lucide-react'
import { Button, Check } from '@/ui'
import { toast, toastError } from '@/ui/Toasts'
import { api, type Instance } from '@/lib/api'
import { live, useGames, useLog } from '@/lib/store'
import { cn } from '@/lib/utils'

/** The game's console: what it printed, and why it stopped when it stopped badly. */
export function LogTab({ instance }: { instance: Instance }) {
  const lines = useLog(instance.id)
  const games = useGames()
  const game = games[instance.id]
  const [query, setQuery] = useState('')
  const [problemsOnly, setProblemsOnly] = useState(false)
  const box = useRef<HTMLDivElement | null>(null)
  const pinned = useRef(true)

  useEffect(() => void live.loadLog(instance.id), [instance.id])

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase()
    return lines.filter((l) => (!problemsOnly || l.stream === 'err' || l.stream === 'warn') && (!q || l.line.toLowerCase().includes(q)))
  }, [lines, query, problemsOnly])

  // Follow new lines while the view is at the bottom.
  useLayoutEffect(() => {
    const el = box.current
    if (el && pinned.current) el.scrollTop = el.scrollHeight
  }, [shown])

  const exit = game && !game.running ? game.exit : null

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 px-6 py-3">
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <label className="kryo-pill flex h-9 w-64 items-center gap-2 border border-border px-3 focus-within:border-foreground">
          <Search className="size-3.5 text-muted-foreground" />
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Find in the log" className="min-w-0 grow bg-transparent text-xs outline-none placeholder:text-muted-foreground" />
        </label>
        <Check checked={problemsOnly} onChange={setProblemsOnly} label="Warnings and errors only" />
        <div className="ml-auto flex gap-2">
          <Button
            size="sm"
            disabled={!lines.length}
            onClick={() =>
              void navigator.clipboard
                .writeText(lines.map((l) => l.line).join('\n'))
                .then(() => toast('Log copied.'))
                .catch(toastError)
            }
          >
            <Copy className="size-3" /> Copy
          </Button>
          <Button size="sm" onClick={() => void api.openFolder(instance.id, 'logs').catch(toastError)}>
            <FolderOpen className="size-3" /> Game logs
          </Button>
        </div>
      </div>

      {exit?.hint ? (
        <div role="alert" className="kryo-radius flex shrink-0 items-start gap-3 border border-destructive/50 bg-destructive/10 px-4 py-3">
          <AlertTriangle className="mt-0.5 size-4 shrink-0 text-destructive" />
          <div className="grid grow gap-1">
            <p className="text-xs leading-relaxed text-foreground">{exit.hint}</p>
            {exit.code !== null ? <p className="text-[10px] uppercase tracking-wider text-muted-foreground">Exit code {exit.code}</p> : null}
          </div>
          {exit.crashReport ? (
            <Button size="sm" onClick={() => void api.openCrashReport(instance.id, exit.crashReport!).catch(toastError)}>
              <FileWarning className="size-3" /> Crash report
            </Button>
          ) : null}
        </div>
      ) : null}

      <div
        ref={box}
        onScroll={(e) => {
          const el = e.currentTarget
          pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40
        }}
        className="kryo-radius kryo-log min-h-0 grow overflow-auto border border-border bg-card p-3 text-[11px] leading-[1.55]"
      >
        {shown.length === 0 ? (
          <p className="text-muted-foreground">{lines.length ? 'No line matches.' : 'Nothing yet. The game writes here once it starts.'}</p>
        ) : (
          shown.map((l, i) => (
            <div key={i} className={cn(l.stream === 'err' ? 'text-destructive' : l.stream === 'warn' ? 'text-warning' : l.stream === 'launcher' ? 'text-info' : 'text-foreground/85')}>
              {l.line}
            </div>
          ))
        )}
      </div>
    </div>
  )
}
