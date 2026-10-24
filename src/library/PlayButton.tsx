import { useEffect, useState } from 'react'
import { Download, Play, Square, X } from 'lucide-react'
import { AsciiBar, Button, Caption } from '@/ui'
import { toastError } from '@/ui/Toasts'
import { api, type Instance } from '@/lib/api'
import { taskFor, taskFraction, useGames, useTasks } from '@/lib/store'
import { speed } from '@/lib/units'

/**
 * The one white pill on an instance: Install, Play, or Stop, and while a job
 * runs, its progress instead.
 */
export function PlayButton({ instance, size = 'lg', onPlay }: { instance: Instance; size?: 'md' | 'lg'; onPlay?: (server?: string) => Promise<void> | void }) {
  const tasks = useTasks()
  const games = useGames()
  const task = taskFor(tasks, instance.id)
  const game = games[instance.id]
  const [pressed, setPressed] = useState(false)
  useEffect(() => setPressed(false), [task?.id, game?.running])

  if (game?.running) {
    return (
      <div className="flex items-center gap-4">
        <Button size={size} variant="danger" onClick={() => void api.stop(instance.id).catch(toastError)}>
          <Square className="size-4" /> Stop
        </Button>
        <Playing since={game.started} />
      </div>
    )
  }
  if (task) {
    const f = taskFraction(task)
    return (
      <div className="flex min-w-0 items-center gap-4">
        <div className="grid min-w-0 gap-1.5">
          <AsciiBar fraction={f} cells={size === 'lg' ? 26 : 16} />
          <Caption className="truncate">
            {task.stage}
            {task.speed > 0 ? ` · ${speed(task.speed)}` : ''}
          </Caption>
        </div>
        <Button size="sm" onClick={() => void api.cancelTask(task.id)}>
          <X className="size-3" /> Cancel
        </Button>
      </div>
    )
  }
  const play = async () => {
    setPressed(true)
    try {
      if (onPlay) await onPlay()
      else await api.launch(instance.id)
    } catch (e) {
      toastError(e)
      setPressed(false)
    }
  }
  return (
    <Button size={size} variant="primary" disabled={pressed} onClick={() => void play()} className={size === 'lg' ? 'min-w-40' : undefined}>
      {instance.installed ? <Play className="size-4 fill-current" /> : <Download className="size-4" />}
      {instance.installed ? 'Play' : 'Install & play'}
    </Button>
  )
}

function Playing({ since }: { since: number | null }) {
  const [, tick] = useState(0)
  useEffect(() => {
    const t = window.setInterval(() => tick((n) => n + 1), 15_000)
    return () => window.clearInterval(t)
  }, [])
  const minutes = since ? Math.max(0, Math.floor((Date.now() / 1000 - since) / 60)) : 0
  return (
    <span className="flex items-center gap-2 text-[10px] uppercase tracking-wider text-muted-foreground">
      <span className="size-1.5 rounded-full bg-success" />
      Playing{minutes ? ` · ${minutes} min` : ''}
    </span>
  )
}
