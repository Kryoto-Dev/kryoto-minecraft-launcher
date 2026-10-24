import { live } from '@/lib/store'

/**
 * Starting a game needs a name to play under. With none yet, the shell asks
 * for one (and saves it as the first profile) before the game starts; the
 * shell registers how it asks.
 */

type Ask = (then: () => void) => void
let ask: Ask | null = null

export function setProfilePrompt(fn: Ask | null) {
  ask = fn
}

/** Run `then` once there is a profile: now, or after the player picks a name. */
export function withProfile(then: () => void) {
  if (live.activeProfile()) then()
  else if (ask) ask(then)
}
