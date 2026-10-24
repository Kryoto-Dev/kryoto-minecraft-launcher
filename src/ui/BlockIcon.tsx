import { useMemo, type CSSProperties } from 'react'
import { cn } from '@/lib/utils'

/**
 * An instance's icon: a small isometric block, drawn here pixel by pixel (no
 * game textures are copied). Each face is a 6x6 grid of pixels with its own
 * pattern, lit from above-left, in true isometric proportions.
 *
 * The pixels move the way Kryoto's matrix does, in hard steps, never gliding:
 * - `build`: the block assembles, bottom pixels first, once.
 * - `progress` (0..1): pixels fill in with a download; the rest stay a trace.
 * - `glint`: a stepped shimmer runs over the faces (playing, hover).
 * Reduced motion (the system's or the launcher's) leaves the block still.
 */

const N = 6

type Face = 'top' | 'left' | 'right'
type Pattern = (face: Face, u: number, v: number, noise: number) => string

type Block = { label: string; pattern: Pattern }

const pick = (colors: string[], noise: number) => colors[Math.min(colors.length - 1, Math.floor(noise * colors.length))]!

/** The dirt sides most natural blocks share, with a fringe of `fringe` on top. */
const fringed = (top: string[], fringe: string[], dirt = ['#6b4a2b', '#7a5634', '#86603a', '#5e4127']): Pattern => (face, _u, v, n) => {
  if (face === 'top') return pick(top, n)
  if (v === 0 || (v === 1 && n > 0.55)) return pick(fringe, n)
  return pick(dirt, n)
}

const uniform = (colors: string[]): Pattern => (_f, _u, _v, n) => pick(colors, n)

export const BLOCKS: Record<string, Block> = {
  grass: { label: 'Grass', pattern: fringed(['#5d9e3f', '#6aab4a', '#78b956', '#4f8c34'], ['#5d9e3f', '#6aab4a']) },
  snow: { label: 'Snow', pattern: fringed(['#f4f7f9', '#e6ecf0', '#ffffff'], ['#f4f7f9', '#e6ecf0']) },
  stone: { label: 'Stone', pattern: uniform(['#7d7d7d', '#8d8d8d', '#999999', '#6e6e6e']) },
  sand: { label: 'Sand', pattern: uniform(['#dccd94', '#e4d6a0', '#d1c286', '#e9dcaa']) },
  wood: {
    label: 'Planks',
    pattern: (face, u, v, n) => {
      const plank = face === 'top' ? u : v
      if (plank % 3 === 2) return '#6e4c26'
      return pick(['#a87a45', '#b88a52', '#9c703e'], (n + (((face === 'top' ? v : u) + plank) % 2) * 0.3) % 1)
    },
  },
  leaves: {
    label: 'Leaves',
    pattern: (_f, _u, _v, n) => (n > 0.82 ? '#1f3f17' : pick(['#3a7a2a', '#468a33', '#2f6a22'], n / 0.82)),
  },
  diamond: {
    label: 'Diamond',
    pattern: (face, u, v, n) => {
      const edge = u === 0 || v === 0 || u === N - 1 || v === N - 1
      if (edge) return '#3aa8a0'
      return face === 'top' && (u + v) % 4 === 1 ? '#c9fff9' : pick(['#62d9cf', '#73e6dc', '#55c9bf'], n)
    },
  },
  gold: {
    label: 'Gold',
    pattern: (face, u, v, n) => {
      const edge = u === 0 || v === 0 || u === N - 1 || v === N - 1
      if (edge) return '#c9971e'
      return face === 'top' && (u + v) % 4 === 1 ? '#fff3b0' : pick(['#f0c93b', '#f8d64e', '#e4bb2d'], n)
    },
  },
  redstone: {
    label: 'Redstone',
    pattern: (_f, u, v, n) => ((u * 7 + v * 3) % 5 === 0 ? '#ff6a55' : pick(['#b8281f', '#c8352b', '#a02018'], n)),
  },
  obsidian: {
    label: 'Obsidian',
    pattern: (_f, _u, _v, n) => (n > 0.88 ? '#6b4f9e' : pick(['#1c1529', '#251c37', '#2f2445'], n / 0.88)),
  },
  ice: {
    label: 'Ice',
    pattern: (_f, u, v, n) => ((u + v) % 5 === 0 ? '#dcecff' : pick(['#93bdf0', '#a3c9f5', '#86b0e6'], n)),
  },
  netherrack: { label: 'Netherrack', pattern: uniform(['#7e3434', '#8a3b3b', '#6e2b2b', '#9a4747']) },
  tnt: {
    label: 'TNT',
    pattern: (face, _u, v, n) => {
      if (face === 'top') return pick(['#d8d0c0', '#c9c1b0'], n)
      if (v === 2 || v === 3) return v === 2 ? '#e8e2d4' : '#d8d0c0'
      return pick(['#c8352b', '#b82c22', '#d23f34'], n)
    },
  },
}

export const BLOCK_NAMES = Object.keys(BLOCKS)

/** A stable block for an instance without one. */
export function blockFor(seed: string) {
  let h = 0
  for (const c of seed) h = (h * 31 + c.charCodeAt(0)) >>> 0
  return BLOCK_NAMES[h % BLOCK_NAMES.length] ?? 'grass'
}

/** Deterministic per-pixel noise. */
function noise(face: number, u: number, v: number, salt: number) {
  let x = (face * 73856093) ^ (u * 19349663) ^ (v * 83492791) ^ (salt * 2654435761)
  x = (x ^ (x >>> 13)) * 1274126177
  return ((x ^ (x >>> 16)) >>> 0) / 4294967296
}

function shade(hex: string, factor: number) {
  const n = parseInt(hex.slice(1), 16)
  const f = (c: number) => Math.max(0, Math.min(255, Math.round(c * factor)))
  return `rgb(${f(n >> 16)},${f((n >> 8) & 255)},${f(n & 255)})`
}

// True isometric: the top is a rhombus half as tall as it is wide, and the
// sides are as tall as an edge. In a 32 x 34 box.
const W = 13.86 // half the width
const H = 8 // half the top's height
const S = 16 // side height
const CX = 16
const TOP = 1

function corner(face: Face, a: number, b: number): [number, number] {
  const x = a / N
  const y = b / N
  if (face === 'top') return [CX + (x - y) * W, TOP + (x + y) * H]
  if (face === 'left') return [CX - W + x * W, TOP + H + x * H + y * S]
  return [CX + x * W, TOP + 2 * H - x * H + y * S]
}

const LIGHT: Record<Face, number> = { top: 1.06, left: 0.82, right: 0.62 }
const FACES: Face[] = ['left', 'right', 'top']

type Pixel = { key: string; points: string; fill: string; order: number; glint: number }

function pixels(name: string): Pixel[] {
  const block = BLOCKS[name] ?? BLOCKS.grass!
  const out: Pixel[] = []
  FACES.forEach((face, fi) => {
    for (let v = 0; v < N; v++) {
      for (let u = 0; u < N; u++) {
        const n = noise(fi, u, v, name.length)
        const pts = [corner(face, u, v), corner(face, u + 1, v), corner(face, u + 1, v + 1), corner(face, u, v + 1)]
        // Built from the bottom up: the sides' lowest rows first, the top last.
        const order = face === 'top' ? 2 * N * N + (2 * N - 2 - u - v) * N + u : (N - 1 - v) * 2 * N + (face === 'left' ? u : N + u)
        out.push({
          key: `${face}${u}${v}`,
          points: pts.map((p) => `${p[0].toFixed(2)},${p[1].toFixed(2)}`).join(' '),
          fill: shade(block.pattern(face, u, v, n), LIGHT[face]),
          order,
          glint: noise(fi + 3, u, v, 7),
        })
      }
    }
  })
  const sorted = [...out].sort((a, b) => a.order - b.order)
  sorted.forEach((p, i) => (p.order = i))
  return out
}

export function BlockIcon({
  name,
  className,
  build = false,
  progress,
  glint = false,
}: {
  name: string | null | undefined
  className?: string
  build?: boolean
  progress?: number | null
  glint?: boolean
}) {
  const key = name && BLOCKS[name] ? name : 'grass'
  const list = useMemo(() => pixels(key), [key])
  const total = list.length
  const filled = progress === undefined || progress === null ? total : Math.round(Math.max(0, Math.min(1, progress)) * total)
  return (
    <svg viewBox="0 0 32 34" className={cn('kryo-block shrink-0', build && 'kryo-block-build', glint && 'kryo-block-glint', className)} aria-hidden shapeRendering="crispEdges">
      {list.map((p) => (
        <polygon
          key={p.key}
          points={p.points}
          fill={p.fill}
          stroke={p.fill}
          strokeWidth={0.25}
          opacity={p.order < filled ? 1 : 0.14}
          style={{ '--i': p.order, '--g': p.glint } as CSSProperties}
        />
      ))}
    </svg>
  )
}
