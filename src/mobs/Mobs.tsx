import { useEffect, useRef, useState } from 'react'
import { api, type Mob } from '@/lib/api'
import { useInstances } from '@/lib/store'

const pick = <T,>(list: T[]) => list[Math.floor(Math.random() * list.length)]!

/**
 * Baby mobs walking along the bottom of the window. They need a game version
 * with baby mobs installed (their shapes and looks come from there), so they
 * turn up once there is one. A different mix every start; click one and it
 * hops.
 */
export function Mobs({ count, friendlyOnly }: { count: number; friendlyOnly: boolean }) {
  const canvas = useRef<HTMLCanvasElement | null>(null)
  const [mobs, setMobs] = useState<Mob[] | null>(null)
  const installed = (useInstances() ?? []).filter((i) => i.installed).length

  useEffect(() => {
    let cancelled = false
    void (async () => {
      try {
        const all = (await api.mobsAvailable()).filter((m) => !friendlyOnly || !m.mean)
        if (!all.length) return setMobs(null)
        // Different kinds first; repeats only when there are more places than kinds.
        const shuffled = [...all].sort(() => Math.random() - 0.5)
        const chosen = Array.from({ length: Math.max(1, Math.min(count, 12)) }, (_, i) => shuffled[i % shuffled.length]!)
        const loaded = await api.mobsLoad(chosen.map((m) => [m.id, pick(m.variants)]))
        if (!cancelled) setMobs(loaded)
      } catch {
        // No version with baby mobs installed yet, or one this reader can't read.
        if (!cancelled) setMobs(null)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [installed, count, friendlyOnly])

  useEffect(() => {
    const el = canvas.current
    if (!el || !mobs?.length) return
    const still = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches || document.documentElement.dataset.motion === 'reduced'
    let scene: Awaited<ReturnType<typeof import('./scene').startMobs>> | null = null
    let disposed = false
    void import('./scene').then(({ startMobs }) =>
      startMobs(el, mobs, still).then((s) => {
        if (disposed) s.dispose()
        else scene = s
      }),
    )
    // The canvas lets clicks through to the app; a click that lands on a mob also makes it hop.
    const onDown = (e: PointerEvent) => {
      const r = el.getBoundingClientRect()
      scene?.tap(e.clientX - r.left, e.clientY - r.top)
    }
    window.addEventListener('pointerdown', onDown)
    return () => {
      disposed = true
      window.removeEventListener('pointerdown', onDown)
      scene?.dispose()
    }
  }, [mobs])

  if (!mobs?.length) return null
  return <canvas ref={canvas} aria-hidden className="pointer-events-none absolute inset-x-0 bottom-0 z-30 h-[150px] w-full" />
}
