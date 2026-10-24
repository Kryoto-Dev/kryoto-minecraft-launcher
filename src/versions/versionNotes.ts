import type { VersionKind } from '@/lib/api'

/**
 * What a player should know before picking a version: honest limits of old
 * versions on today's computers and services, not marketing.
 */
export function versionNotes(id: string, kind: VersionKind): string[] {
  const notes: string[] = []
  if (kind === 'snapshot') notes.push('A snapshot is a test build. Back up a world before opening it here; snapshots can change or break worlds.')
  if (kind === 'old_alpha' || kind === 'old_beta') {
    notes.push('A very old version. It starts and plays, but the services it used are gone: skins and capes do not load, and some sounds may be missing.')
    notes.push('Few servers run it any more. Singleplayer works as it always did.')
  }
  if (id.startsWith('rd-') || id.startsWith('c0.') || id.startsWith('inf-')) notes.push('From the first months of Minecraft: a tech demo, with no survival mode or saving as you know it.')
  if (kind === 'old_alpha' || kind === 'old_beta' || /^1\.[0-5](\.|$)/.test(id)) notes.push('Uses texture packs instead of resource packs.')
  if (/^1\.(1[3-6])(\.|$)/.test(id)) notes.push('Some graphics drivers have trouble with this era of versions. If it crashes on start, update your graphics driver.')
  return notes
}
