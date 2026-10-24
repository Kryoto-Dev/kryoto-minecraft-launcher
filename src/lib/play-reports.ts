import { call, errorText } from '@/lib/bridge'
import { logError, logWarn } from '@/lib/log'

/**
 * Play sessions on their way to kryo.to, kept until they arrive.
 *
 * A session is reported through the Store page's own session, so a game closed
 * while that page was not loaded ("Open kryo.to first", 16 reports in 0.2.3)
 * used to lose its play time for good. It waits here instead, in this PC's
 * storage, and goes the next time a report can be made. The key on each makes
 * a second send harmless: kryo.to counts a session once.
 */

type PendingPlay = { slug: string; startedAt: number; seconds: number; key: string }

const STORE_KEY = 'kryoto.pendingPlays'
/** Older than this and the site would not take it anyway. */
const MAX_AGE_SECS = 14 * 86_400

function read(): PendingPlay[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(STORE_KEY) ?? '[]') as PendingPlay[]
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

function write(list: PendingPlay[]) {
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify(list.slice(-200)))
  } catch {
    // Storage full or off: the session is lost, as it was before.
  }
}

/** Expected while the Store is not loaded, or for a game kryo.to does not list. */
export function isExpectedReportError(e: unknown): boolean {
  const text = errorText(e)
  return text.startsWith('Open kryo.to first') || text.startsWith('Not a kryo.to game')
}

/** Send one session; keep it for later when kryo.to is not reachable yet. */
export async function reportPlay(play: PendingPlay): Promise<void> {
  try {
    await call('store_report_play', play)
  } catch (e) {
    if (errorText(e).startsWith('Open kryo.to first')) {
      write([...read().filter((p) => p.key !== play.key), play])
      logWarn('playtime', `${play.slug}: kept for later (${errorText(e)})`)
    } else if (isExpectedReportError(e)) {
      logWarn('playtime', `${play.slug}: ${errorText(e)}`)
    } else {
      logError('playtime', e)
    }
  }
}

/** Try every kept session again. Called on a timer; quiet when there is nothing. */
export async function flushPlays(): Promise<void> {
  const now = Math.floor(Date.now() / 1000)
  const list = read().filter((p) => now - p.startedAt < MAX_AGE_SECS)
  if (list.length === 0) return
  const left: PendingPlay[] = []
  for (const play of list) {
    try {
      await call('store_report_play', play)
    } catch (e) {
      if (errorText(e).startsWith('Open kryo.to first')) left.push(play)
    }
  }
  write(left)
}
