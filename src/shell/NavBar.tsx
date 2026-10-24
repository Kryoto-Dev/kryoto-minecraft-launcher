import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { Download, Gamepad2, Layers, Settings, type LucideIcon } from 'lucide-react'
import { cn } from '@/lib/utils'

export type Tab = 'play' | 'versions' | 'downloads' | 'settings'

const TABS: { id: Tab; label: string; icon: LucideIcon }[] = [
  { id: 'play', label: 'Play', icon: Gamepad2 },
  { id: 'versions', label: 'Versions', icon: Layers },
  { id: 'downloads', label: 'Downloads', icon: Download },
  { id: 'settings', label: 'Settings', icon: Settings },
]

/**
 * kryo.to's segmented pill, as Kryoto Desktop's NavBar draws it: the white
 * segment slides to where you are. The right side carries the page's actions.
 */
export function NavBar({ current, onChange, badges, right }: { current: Tab; onChange: (t: Tab) => void; badges?: Partial<Record<Tab, number>>; right?: ReactNode }) {
  const track = useRef<HTMLDivElement | null>(null)
  const slots = useRef(new Map<Tab, HTMLElement>())
  const [pill, setPill] = useState<{ left: number; width: number } | null>(null)
  const measure = useCallback(() => {
    const el = slots.current.get(current)
    if (!el) return setPill(null)
    setPill((p) => (p && p.left === el.offsetLeft && p.width === el.offsetWidth ? p : { left: el.offsetLeft, width: el.offsetWidth }))
  }, [current])
  useLayoutEffect(measure)
  useEffect(() => {
    if (!track.current) return
    const ro = new ResizeObserver(measure)
    ro.observe(track.current)
    return () => ro.disconnect()
  }, [measure])

  return (
    <nav aria-label="Main" className="relative z-40 flex h-14 shrink-0 items-center gap-3 border-b border-border bg-background px-3">
      <div ref={track} role="tablist" className="kryo-pill relative flex items-center gap-1 border border-border bg-card p-1">
        {pill ? (
          <span
            aria-hidden
            className="kryo-pill pointer-events-none absolute bottom-1 top-1 bg-primary transition-[left,width] duration-300 ease-[cubic-bezier(0.32,0.72,0,1)]"
            style={{ left: pill.left, width: pill.width }}
          />
        ) : null}
        {TABS.map((t) => {
          const Icon = t.icon
          const badge = badges?.[t.id]
          return (
            <button
              key={t.id}
              ref={(el) => {
                if (el) slots.current.set(t.id, el)
                else slots.current.delete(t.id)
              }}
              type="button"
              role="tab"
              aria-selected={current === t.id}
              onClick={() => onChange(t.id)}
              className={cn(
                'kryo-pill relative z-10 flex h-9 items-center gap-2 px-5 text-xs font-bold uppercase tracking-[0.2em] transition-colors',
                current === t.id ? 'text-primary-foreground' : 'text-muted-foreground hover:text-foreground',
              )}
            >
              <Icon className="size-3.5" aria-hidden />
              {t.label}
              {badge ? (
                <span className={cn('kryo-pill grid h-4 min-w-4 place-items-center px-1 text-[9px] tracking-normal', current === t.id ? 'bg-primary-foreground text-primary' : 'bg-primary text-primary-foreground')}>
                  {badge}
                </span>
              ) : null}
            </button>
          )
        })}
      </div>
      <div className="flex min-w-0 grow items-center justify-end gap-2">{right}</div>
    </nav>
  )
}
