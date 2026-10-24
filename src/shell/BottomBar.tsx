import { Download } from 'lucide-react'
import { AsciiBar } from '@/ui'
import type { Instance } from '@/lib/api'
import { taskFraction, useGames, useTasks } from '@/lib/store'
import { speed } from '@/lib/units'

/** Desktop's bottom strip: the download in the middle, what is running on the right. */
export function BottomBar({ instances, onDownloads, onOpen }: { instances: Instance[]; onDownloads: () => void; onOpen: (id: string) => void }) {
  const tasks = useTasks()
  const games = useGames()
  const running = tasks.filter((t) => t.state === 'running')
  const first = running[0]
  const playing = Object.values(games).filter((g) => g.running)
  const name = (id: string) => instances.find((i) => i.id === id)?.name ?? id

  return (
    <footer className="flex h-9 shrink-0 items-center gap-4 border-t border-border bg-background px-3 text-[10px] uppercase tracking-wider">
      <div className="flex min-w-0 grow justify-center">
        {first ? (
          <button type="button" onClick={onDownloads} className="kryo-square flex min-w-0 items-center gap-3 uppercase text-muted-foreground hover:text-foreground">
            <Download className="size-3 shrink-0" />
            <span className="max-w-56 truncate text-foreground">{first.title}</span>
            <AsciiBar fraction={taskFraction(first)} cells={14} showPct className="text-[10px]" />
            {first.speed > 0 ? <span className="tabular-nums">{speed(first.speed)}</span> : <span className="max-w-40 truncate normal-case tracking-normal">{first.stage}</span>}
            {running.length > 1 ? <span>+{running.length - 1}</span> : null}
          </button>
        ) : null}
      </div>
      <div className="flex items-center gap-3">
        {playing.map((g) => (
          <button key={g.instance} type="button" onClick={() => onOpen(g.instance)} className="kryo-square flex items-center gap-1.5 uppercase text-foreground">
            <span className="size-1.5 rounded-full bg-success" /> {name(g.instance)}
          </button>
        ))}
      </div>
    </footer>
  )
}
