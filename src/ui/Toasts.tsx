import { useSyncExternalStore } from 'react'
import { X } from 'lucide-react'
import { errorText } from '@/lib/bridge'
import { cn } from '@/lib/utils'

/** Short notes in the corner, as Kryoto Desktop shows them. Errors stay until closed. */

type Toast = { id: number; text: string; tone: 'info' | 'error' }

let toasts: Toast[] = []
let next = 1
const listeners = new Set<() => void>()
const emit = () => listeners.forEach((l) => l())

export function toast(text: string, tone: Toast['tone'] = 'info') {
  const id = next++
  toasts = [...toasts.slice(-3), { id, text, tone }]
  emit()
  if (tone === 'info') window.setTimeout(() => dismiss(id), 4500)
}

export const toastError = (e: unknown) => toast(errorText(e), 'error')

function dismiss(id: number) {
  toasts = toasts.filter((t) => t.id !== id)
  emit()
}

export function Toasts() {
  const list = useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => void listeners.delete(l)
    },
    () => toasts,
  )
  return (
    <div className="pointer-events-none fixed bottom-12 right-4 z-[600] grid w-96 max-w-[calc(100vw-32px)] gap-2">
      {list.map((t) => (
        <div
          key={t.id}
          role={t.tone === 'error' ? 'alert' : 'status'}
          className={cn(
            'kryo-toast kryo-radius pointer-events-auto flex items-start gap-3 border bg-popover px-4 py-3 text-xs leading-relaxed shadow-2xl shadow-black/60',
            t.tone === 'error' ? 'border-destructive/60 text-foreground' : 'border-border text-foreground',
          )}
        >
          <span className="grow whitespace-pre-line">{t.text}</span>
          <button type="button" aria-label="Close" onClick={() => dismiss(t.id)} className="kryo-square -mr-1 text-muted-foreground hover:text-foreground">
            <X className="size-3.5" />
          </button>
        </div>
      ))}
    </div>
  )
}
