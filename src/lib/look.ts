/**
 * The look: kryo.to's palettes, corners and type face, applied the same way
 * Kryoto Desktop's `applyLook` does (src/lib/settings.ts there), so a player
 * who picked Amber on the site gets the same Amber here.
 */

export const PALETTES = [
  { value: 'oled', label: 'OLED' },
  { value: 'amber', label: 'Amber' },
  { value: 'emerald', label: 'Emerald' },
  { value: 'nord', label: 'Nord' },
  { value: 'sepia', label: 'Sepia' },
  { value: 'blossom', label: 'Blossom' },
]

export const RADII = [
  { value: 'sharp', label: 'Sharp' },
  { value: 'soft', label: 'Soft' },
  { value: 'rounded', label: 'Rounded' },
  { value: 'round', label: 'Round' },
  { value: 'pill', label: 'Pill' },
]

export const FONTS = [
  { value: 'teletext', label: 'Teletext' },
  { value: 'mono', label: 'Mono' },
]

export function applyLook(s: { palette: string; radius: string; font: string }) {
  const html = document.documentElement
  html.classList.add('dark')
  html.style.colorScheme = 'dark'
  html.dataset.palette = s.palette || 'oled'
  html.dataset.radius = s.radius || 'pill'
  if (s.font === 'mono') html.dataset.font = 'mono'
  else delete html.dataset.font
}
