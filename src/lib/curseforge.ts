import { useEffect, useState } from 'react'
import { api } from '@/lib/api'

/** Whether this build can reach CurseForge (release builds carry its key). Asked once. */
let known: Promise<boolean> | null = null
let answer = false

export function useCurseForge() {
  const [available, setAvailable] = useState(answer)
  useEffect(() => {
    known ??= api.curseforgeAvailable().catch(() => false)
    let cancelled = false
    void known.then((a) => {
      answer = a
      if (!cancelled) setAvailable(a)
    })
    return () => {
      cancelled = true
    }
  }, [])
  return available
}
