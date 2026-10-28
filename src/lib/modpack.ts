import { api } from '@/lib/api'

/**
 * A modpack, which becomes an instance of its own when dropped: Modrinth's
 * .mrpack, or a CurseForge .zip (told from a resource pack by its manifest).
 */
export async function isModpack(path: string) {
  const lower = path.toLowerCase()
  if (lower.endsWith('.mrpack')) return true
  return lower.endsWith('.zip') && (await api.isModpack(path).catch(() => false))
}

/** Split dropped files into modpacks and the rest. */
export async function splitModpacks(paths: string[]) {
  const packs = await Promise.all(paths.map(isModpack))
  return { packs: paths.filter((_, i) => packs[i]), other: paths.filter((_, i) => !packs[i]) }
}
