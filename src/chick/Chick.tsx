import { useEffect, useRef, useState } from 'react'
import { api } from '@/lib/api'
import { useInstances } from '@/lib/store'

const VARIANTS = ['chicken_temperate_baby', 'chicken_warm_baby', 'chicken_cold_baby']

/**
 * A baby chicken that walks along the bottom of the window. It needs a game
 * version with baby chicks installed (its texture comes from there), so it
 * turns up once there is one. Click it and it hops.
 */
export function Chick() {
  const canvas = useRef<HTMLCanvasElement | null>(null)
  const [texture, setTexture] = useState<string | null>(null)
  const installed = (useInstances() ?? []).filter((i) => i.installed).length

  useEffect(() => {
    let cancelled = false
    const variant = VARIANTS[Math.floor(Math.random() * VARIANTS.length)]!
    api
      .mobTexture(variant)
      .catch(() => api.mobTexture(VARIANTS[0]!))
      .then((t) => !cancelled && setTexture(t))
      .catch(() => {
        /* No version with chicks installed yet. */
      })
    return () => {
      cancelled = true
    }
  }, [installed])

  useEffect(() => {
    const el = canvas.current
    if (!el || !texture) return
    const still = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches || document.documentElement.dataset.motion === 'reduced'
    let scene: Awaited<ReturnType<typeof import('./scene').startChick>> | null = null
    let disposed = false
    void import('./scene').then(({ startChick }) =>
      startChick(el, texture, still).then((s) => {
        if (disposed) s.dispose()
        else scene = s
      }),
    )
    // The canvas lets clicks through to the app; a click that lands on the chick also makes it hop.
    const onDown = (e: PointerEvent) => {
      const r = el.getBoundingClientRect()
      if (scene?.hit(e.clientX - r.left, e.clientY - r.top)) scene.hop()
    }
    window.addEventListener('pointerdown', onDown)
    return () => {
      disposed = true
      window.removeEventListener('pointerdown', onDown)
      scene?.dispose()
    }
  }, [texture])

  if (!texture) return null
  return <canvas ref={canvas} aria-hidden className="pointer-events-none absolute inset-x-0 bottom-0 z-30 h-[120px] w-full" />
}
