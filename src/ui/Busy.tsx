import { useEffect, useRef } from 'react'
import { Matrix } from './Matrix'

/**
 * Kryoto's "working" mark: the 3x3 matrix in its `busy` state, a three-dot
 * snake stepping clockwise round the edge.
 *
 * kryo.to's components/ui/busy.tsx, copied. A drop-in for an icon: size it
 * with `size-*`, colour it with `text-*`. Decorative by default; give it a
 * `label` when it stands alone.
 *
 * One more thing, for anyone who waits long enough to notice. A mark that has
 * been turning for a while - somewhere between 25 and 45 seconds - stops for
 * one beat and spells a K, then carries on. Once per mark, never with motion
 * reduced, and only ever on a wait long enough to be watched.
 */
export function Busy({ className, label }: { className?: string; label?: string }) {
  const ref = useRef<HTMLSpanElement>(null)

  useEffect(() => {
    if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return
    if (document.documentElement.dataset.motion === 'reduced') return
    let off = 0
    const on = window.setTimeout(
      () => {
        const el = ref.current
        if (!el) return
        el.dataset.wink = 'k'
        off = window.setTimeout(() => delete el.dataset.wink, 330)
      },
      25_000 + Math.random() * 20_000,
    )
    return () => {
      window.clearTimeout(on)
      window.clearTimeout(off)
    }
  }, [])

  return (
    <span ref={ref} className="contents">
      <Matrix state="busy" label={label} className={className} />
    </span>
  )
}
