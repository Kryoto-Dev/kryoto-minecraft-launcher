// Render Kryoto Minecraft Launcher's brand files from code: `pnpm brand`.
//
// Kryoto Desktop's scripts/brand.mjs, with one change: where Desktop puts the
// K// on a screen, the launcher prints it on the front of a grass block. Same
// tile and mark (src/ui/ascii/mark.ts, traced from kryo.to's); the block's
// green top is what tells it apart from Desktop and Forge at every size,
// down to the taskbar's. Writes:
//
//   src-tauri/icons/       icon.png, 32x32.png, 128x128.png, 128x128@2x.png, icon.ico
//   src-tauri/installer/   header.bmp (150x57), sidebar.bmp (164x314)
//   docs/logo.png          the mark for the README
//
// Deterministic: same code in, same pixels out.

import sharp from 'sharp'
import { mkdirSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { asciiSvg, gridSize, WORDMARK } from '../src/ui/ascii/cells.ts'
import { MARK_H, MARK_PATH, MARK_W } from '../src/ui/ascii/mark.ts'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const icons = path.join(root, 'src-tauri', 'icons')
const installer = path.join(root, 'src-tauri', 'installer')
const docs = path.join(root, 'docs')
for (const d of [icons, installer, docs]) mkdirSync(d, { recursive: true })

const INK = '#ededed'
const TILE = '#0a0a0a'

/** The block: an isometric cube's outline as a double line, and its top face. */
function block(cx, cy, r, t, gap, ink = INK, opacity = 1) {
  const c = Math.cos(Math.PI / 6)
  const stroke = `fill="none" stroke="${ink}" stroke-opacity="${opacity}" stroke-width="${t}" stroke-linejoin="round"`
  const lines = []
  for (const i of [0, 1]) {
    const k = r - i * (t + gap) * 1.6
    const p = [
      [cx, cy - k],
      [cx + k * c, cy - k / 2],
      [cx + k * c, cy + k / 2],
      [cx, cy + k],
      [cx - k * c, cy + k / 2],
      [cx - k * c, cy - k / 2],
    ]
    lines.push(`<polygon points="${p.map((q) => q.join(',')).join(' ')}" ${stroke}/>`)
  }
  const inner = r - (t + gap) * 1.6
  lines.push(`<polyline points="${cx - inner * c},${cy - inner / 2} ${cx},${cy} ${cx + inner * c},${cy - inner / 2}" ${stroke}/>`)
  return lines.join('')
}

/** The same pixels every run: a tiny LCG seeded per face. */
function shades(seed) {
  let s = seed >>> 0
  return () => ((s = (Math.imul(s, 1664525) + 1013904223) >>> 0) / 2 ** 32)
}

const GRASS = ['#5fa83b', '#58a034', '#67b142', '#4f9530', '#6cb846']
const DIRT = ['#7a5233', '#6e4a2e', '#86603d', '#5f3f27', '#734d30']
const N = 8

/** A face's colour at cell (i, j): grass on top, dirt with a grass fringe on the sides. */
function cellColors(side, seed) {
  const rnd = shades(seed)
  const pick = (list) => list[Math.floor(rnd() * list.length)]
  const cells = []
  for (let j = 0; j < N; j++) {
    for (let i = 0; i < N; i++) {
      const grass = !side || j === 0 || (j === 1 && rnd() < 0.55) || (j === 2 && rnd() < 0.12)
      cells.push(pick(grass ? GRASS : DIRT))
    }
  }
  return cells
}

/** A colour darkened for a face turned from the light. */
function dim(hex, f) {
  const n = parseInt(hex.slice(1), 16)
  const c = [n >> 16, (n >> 8) & 255, n & 255].map((v) => Math.round(v * f))
  return `rgb(${c.join(',')})`
}

/**
 * A grass block in isometric view, N x N pixels to a face, around (cx, cy)
 * with `r` from the centre to each corner. Each face is drawn in a unit square
 * mapped onto it, so the K// printed on the left face lies flat on it.
 */
function grassBlock(cx, cy, r, { mark = true, outline = 0 } = {}) {
  const c = Math.cos(Math.PI / 6) * r
  // origin, u (along the face's top edge), v (down the face): a unit square onto each face.
  const faces = [
    { m: [c, -r / 2, c, r / 2, cx - c, cy - r / 2], side: false, light: 1, seed: 7 },
    { m: [c, r / 2, 0, r, cx - c, cy - r / 2], side: true, light: 0.86, seed: 11 },
    { m: [c, -r / 2, 0, r, cx, cy], side: true, light: 0.64, seed: 23 },
  ]
  const px = 1 / N
  let out = ''
  for (const f of faces) {
    const cells = cellColors(f.side, f.seed)
    let rects = ''
    cells.forEach((col, n) => {
      const i = n % N
      const j = Math.floor(n / N)
      // A hair over a pixel each way, so no seam shows between neighbours.
      rects += `<rect x="${i * px - 0.002}" y="${j * px - 0.002}" width="${px + 0.004}" height="${px + 0.004}" fill="${dim(col, f.light)}"/>`
    })
    out += `<g transform="matrix(${f.m.join(' ')})">${rects}</g>`
  }
  if (mark) {
    // On the left face, below the grass: the mark and its shadow a pixel down-right.
    const w = 0.78
    const k = w / MARK_W
    const h = MARK_H * k
    const x = (1 - w) / 2
    const y = 0.36 + (0.6 - h) / 2
    const left = faces[1].m.join(' ')
    out += `<g transform="matrix(${left})"><path transform="translate(${x + 0.035} ${y + 0.035}) scale(${k})" d="${MARK_PATH}" fill="#000" fill-opacity="0.45"/><path transform="translate(${x} ${y}) scale(${k})" d="${MARK_PATH}" fill="#f4f4f4"/></g>`
  }
  if (outline) {
    const hex = [
      [cx, cy - r],
      [cx + c, cy - r / 2],
      [cx + c, cy + r / 2],
      [cx, cy + r],
      [cx - c, cy + r / 2],
      [cx - c, cy - r / 2],
    ]
    out += `<polygon points="${hex.map((q) => q.join(',')).join(' ')}" fill="none" stroke="#000" stroke-opacity="0.55" stroke-width="${outline}" stroke-linejoin="round"/>`
  }
  return out
}

function iconSvg(size) {
  const S = 1024
  const r = S * 0.22
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${S} ${S}">
  <defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#1a1a1a"/><stop offset="1" stop-color="#060606"/></linearGradient></defs>
  <rect width="${S}" height="${S}" rx="${r}" fill="url(#bg)"/>
  <rect x="3" y="3" width="${S - 6}" height="${S - 6}" rx="${r - 3}" fill="none" stroke="#ffffff" stroke-opacity="0.09" stroke-width="6"/>
  <ellipse cx="${S / 2}" cy="${S * 0.85}" rx="${S * 0.3}" ry="${S * 0.045}" fill="#000" fill-opacity="0.5"/>
  ${grassBlock(S / 2, S * 0.5, S * 0.37)}
</svg>`
}

/**
 * Taskbar and tray sizes: the block alone, filling the square, so it is the
 * green block beside Desktop's and Forge's K// rather than a third K//. The
 * mark comes back at 32 px, where it can be read.
 */
async function smallIcon(size) {
  const S = 512
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${S}" height="${S}" viewBox="0 0 ${S} ${S}">${grassBlock(S / 2, S / 2, S * 0.5, { mark: size >= 32, outline: S * 0.02 })}</svg>`
  return sharp(Buffer.from(svg)).resize(size, size, { kernel: 'lanczos3' }).png({ compressionLevel: 9 }).toBuffer()
}

const icon = (size) => (size <= 48 ? smallIcon(size) : sharp(Buffer.from(iconSvg(size))).png({ compressionLevel: 9 }).toBuffer())
const markArt = (width) =>
  sharp(Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${MARK_W} ${MARK_H}" width="${width}" height="${Math.round((MARK_H * width) / MARK_W)}"><path d="${MARK_PATH}" fill="${INK}"/></svg>`))
    .png()
    .toBuffer()

/** A .ico of PNG layers (Vista and later read PNG-in-ICO). */
function ico(layers) {
  const header = Buffer.alloc(6)
  header.writeUInt16LE(0, 0)
  header.writeUInt16LE(1, 2)
  header.writeUInt16LE(layers.length, 4)
  const dir = Buffer.alloc(16 * layers.length)
  let offset = 6 + dir.length
  layers.forEach(({ size, png }, i) => {
    const e = i * 16
    dir.writeUInt8(size >= 256 ? 0 : size, e)
    dir.writeUInt8(size >= 256 ? 0 : size, e + 1)
    dir.writeUInt8(0, e + 2)
    dir.writeUInt8(0, e + 3)
    dir.writeUInt16LE(1, e + 4)
    dir.writeUInt16LE(32, e + 6)
    dir.writeUInt32LE(png.length, e + 8)
    dir.writeUInt32LE(offset, e + 12)
    offset += png.length
  })
  return Buffer.concat([header, dir, ...layers.map((l) => l.png)])
}

/** A 24-bit BMP, which is what NSIS wants for its pictures. */
async function bmp(pngBuffer) {
  const { data, info } = await sharp(pngBuffer).flatten({ background: TILE }).removeAlpha().raw().toBuffer({ resolveWithObject: true })
  const { width, height } = info
  const rowSize = Math.ceil((width * 3) / 4) * 4
  const pixels = Buffer.alloc(rowSize * height)
  for (let y = 0; y < height; y++) {
    const src = (height - 1 - y) * width * 3
    for (let x = 0; x < width; x++) {
      const s = src + x * 3
      const d = y * rowSize + x * 3
      pixels[d] = data[s + 2]
      pixels[d + 1] = data[s + 1]
      pixels[d + 2] = data[s]
    }
  }
  const header = Buffer.alloc(54)
  header.write('BM', 0)
  header.writeUInt32LE(54 + pixels.length, 2)
  header.writeUInt32LE(54, 10)
  header.writeUInt32LE(40, 14)
  header.writeInt32LE(width, 18)
  header.writeInt32LE(height, 22)
  header.writeUInt16LE(1, 26)
  header.writeUInt16LE(24, 28)
  header.writeUInt32LE(pixels.length, 34)
  header.writeInt32LE(2835, 38)
  header.writeInt32LE(2835, 42)
  return Buffer.concat([header, pixels])
}

async function art(lines, width, color = INK) {
  const g = gridSize(lines)
  return sharp(Buffer.from(asciiSvg(lines, { color })))
    .resize({ width, height: Math.round((g.height * width) / g.width), fit: 'fill' })
    .png()
    .toBuffer()
}

const write = (file, buf) => {
  writeFileSync(file, buf)
  console.log(`${path.relative(root, file)}  ${(buf.length / 1024).toFixed(1)} KB`)
}
write(path.join(icons, 'icon.png'), await icon(1024))
write(path.join(icons, '32x32.png'), await icon(32))
write(path.join(icons, '128x128.png'), await icon(128))
write(path.join(icons, '128x128@2x.png'), await icon(256))
const layers = []
for (const size of [16, 20, 24, 32, 40, 48, 64, 128, 256]) layers.push({ size, png: await icon(size) })
write(path.join(icons, 'icon.ico'), ico(layers))
write(path.join(docs, 'logo.png'), await icon(256))

// The installer's sidebar: a dim field of blocks, the icon and the wordmark.
{
  const W = 164
  const H = 314
  let field = ''
  for (let row = 0; row < 14; row++) {
    for (let col = 0; col < 7; col++) {
      field += block(col * 26 + (row % 2) * 13, row * 22 + 6, 13, 1, 0.6, '#ffffff', 0.06)
    }
  }
  const backdrop = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}"><rect width="${W}" height="${H}" fill="${TILE}"/>${field}<defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${TILE}" stop-opacity="0"/><stop offset="0.6" stop-color="${TILE}" stop-opacity="0.6"/><stop offset="1" stop-color="${TILE}" stop-opacity="1"/></linearGradient></defs><rect width="${W}" height="${H}" fill="url(#g)"/></svg>`
  const mark = await sharp(await icon(120)).png().toBuffer()
  const words = await art(WORDMARK, 118)
  const sidebar = await sharp(Buffer.from(backdrop))
    .composite([
      { input: mark, top: 44, left: 22 },
      { input: words, top: H - 58, left: 23 },
    ])
    .png()
    .toBuffer()
  write(path.join(installer, 'sidebar.bmp'), await bmp(sidebar))
}
{
  const W = 150
  const H = 57
  const mark = await markArt(80)
  const header = await sharp({ create: { width: W, height: H, channels: 3, background: TILE } })
    .composite([{ input: mark, gravity: 'centre' }])
    .png()
    .toBuffer()
  write(path.join(installer, 'header.bmp'), await bmp(header))
}
