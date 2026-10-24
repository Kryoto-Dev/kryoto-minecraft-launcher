import { AlertTriangle, Check as CheckIcon, RotateCw, X } from 'lucide-react'
import { AsciiBar, Button, Caption, Label } from '@/ui'
import { Matrix } from '@/ui/Matrix'
import { toastError } from '@/ui/Toasts'
import { api, retryTask, type Task } from '@/lib/api'
import { taskFraction, useTasks } from '@/lib/store'
import { bytes, speed } from '@/lib/units'

/** Every job: what is downloading now, and what finished or failed. */
export function DownloadsPage() {
  const tasks = useTasks()
  const running = tasks.filter((t) => t.state === 'running')
  // A start that downloaded nothing is not worth a row once it is done.
  const finished = tasks.filter((t) => t.state !== 'running' && !(t.state === 'done' && t.title.startsWith('Starting ') && t.totalBytes === 0)).reverse()

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto grid max-w-4xl gap-8 px-6 py-6">
        <section className="grid gap-3">
          <Label>Now</Label>
          {running.length === 0 ? (
            <p className="text-xs text-muted-foreground">Nothing is downloading. Files that are already here are never downloaded twice.</p>
          ) : (
            running.map((t) => <Running key={t.id} task={t} />)
          )}
        </section>
        {finished.length ? (
          <section className="grid gap-3">
            <div className="flex items-center justify-between">
              <Label>Earlier</Label>
              <Button size="sm" variant="ghost" onClick={() => void api.clearTasks()}>
                Clear
              </Button>
            </div>
            <ul className="grid gap-1">
              {finished.map((t) => (
                <Finished key={t.id} task={t} />
              ))}
            </ul>
          </section>
        ) : null}
      </div>
    </div>
  )
}

function Running({ task }: { task: Task }) {
  const f = taskFraction(task)
  return (
    <div className="kryo-radius grid gap-3 border border-border bg-card p-5">
      <div className="flex items-center gap-3">
        <Matrix state="download" className="size-4" />
        <span className="grow truncate text-sm font-bold">{task.title}</span>
        <Button size="sm" onClick={() => void api.cancelTask(task.id)}>
          <X className="size-3" /> Cancel
        </Button>
      </div>
      <AsciiBar fraction={f} cells={48} />
      <div className="flex flex-wrap gap-x-5 gap-y-1">
        <Caption className="text-foreground/80">{task.stage}</Caption>
        {task.totalFiles > 0 ? (
          <Caption>
            {task.doneFiles.toLocaleString()} of {task.totalFiles.toLocaleString()} files
          </Caption>
        ) : null}
        {task.totalBytes > 0 ? (
          <Caption>
            {bytes(task.doneBytes)} of {bytes(task.totalBytes)}
          </Caption>
        ) : null}
        {task.speed > 0 ? <Caption>{speed(task.speed)}</Caption> : null}
      </div>
    </div>
  )
}

function Finished({ task }: { task: Task }) {
  const failed = task.state === 'failed'
  return (
    <li className="kryo-radius grid gap-2 border border-border px-4 py-3">
      <div className="flex items-center gap-3">
        {failed ? <AlertTriangle className="size-3.5 text-destructive" /> : task.state === 'done' ? <CheckIcon className="size-3.5 text-success" /> : <X className="size-3.5 text-muted-foreground" />}
        <span className="grow truncate text-xs">{task.title}</span>
        <Caption>{task.state === 'done' ? (task.downloaded ? `${bytes(task.downloaded)} downloaded` : 'Nothing to download') : task.state === 'cancelled' ? 'Cancelled' : 'Failed'}</Caption>
        {(failed || task.state === 'cancelled') && task.retry ? (
          <Button size="sm" onClick={() => void retryTask(task).catch(toastError)}>
            <RotateCw className="size-3" /> Try again
          </Button>
        ) : null}
      </div>
      {failed && task.error ? <p className="kryo-log text-[11px] leading-relaxed text-destructive">{task.error}</p> : null}
    </li>
  )
}
