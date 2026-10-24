import { useEffect, useState } from 'react'
import { RotateCw } from 'lucide-react'
import { AsciiBar, Busy, Button, Caption, Modal } from '@/ui'
import { DOWNLOAD_PAGE, explainUpdateError, updateFraction, updates, useUpdates } from '@/lib/updates'
import { openExternal } from '@/lib/window'

/**
 * Offer a new version; never take the decision (Kryoto Desktop's prompt).
 * Installing restarts the launcher, which does not stop a running game, but
 * a download in progress starts over the files it was on.
 */
export function UpdatePrompt({ busy }: { busy: number }) {
  const state = useUpdates()
  const [dismissed, setDismissed] = useState(false)
  useEffect(() => {
    void updates.loadVersion()
    void updates.checkOnLaunch()
  }, [])
  const { update } = state
  if (!update || dismissed) return null
  const working = state.status === 'downloading' || state.status === 'ready'
  const problem = state.status === 'failed' ? explainUpdateError(state.error) : null
  return (
    <Modal
      title="Update available"
      onClose={() => !working && setDismissed(true)}
      footer={
        <>
          <Button onClick={() => setDismissed(true)} disabled={working}>
            Later
          </Button>
          {problem && !problem.retryable ? (
            <Button variant="primary" onClick={() => void openExternal(DOWNLOAD_PAGE)}>
              Download page
            </Button>
          ) : (
            <Button variant="primary" onClick={() => void updates.install()} disabled={working}>
              {state.status === 'failed' ? 'Try again' : 'Update now'}
            </Button>
          )}
        </>
      }
    >
      <div className="grid gap-4">
        <p className="text-xs leading-relaxed">
          Kryoto Minecraft Launcher <b>{update.version}</b> is out (you have {state.current || 'an older one'}). It downloads, installs and restarts in a moment.
        </p>
        {busy > 0 ? <p className="text-[11px] text-muted-foreground">A download is running: it continues from its last finished file after the restart. Games keep running.</p> : null}
        {working ? (
          <div className="grid gap-1.5">
            <AsciiBar fraction={updateFraction(state)} cells={24} />
            <Caption>{state.status === 'ready' ? 'Restarting' : 'Downloading'}</Caption>
          </div>
        ) : null}
        {state.status === 'failed' ? <p className="text-[11px] leading-relaxed text-destructive">{problem?.headline ?? state.error}</p> : null}
      </div>
    </Modal>
  )
}

/** "Check for updates" with its answer, for About. */
export function UpdateCheck() {
  const state = useUpdates()
  useEffect(() => void updates.loadVersion(), [])
  const problem = state.status === 'failed' ? explainUpdateError(state.error) : null
  const line =
    state.status === 'checking'
      ? 'Checking'
      : state.status === 'current'
        ? 'You have the newest version.'
        : state.status === 'available' && state.update
          ? `Version ${state.update.version} is available.`
          : state.status === 'downloading' || state.status === 'ready'
            ? 'Updating'
            : state.status === 'failed'
              ? (problem?.headline ?? state.error ?? 'The check failed.')
              : null
  return (
    <div className="grid justify-items-center gap-2">
      <div className="flex gap-2">
        <Button size="sm" onClick={() => void updates.check()} disabled={state.status === 'checking' || state.status === 'downloading'}>
          {state.status === 'checking' ? <Busy className="size-3" /> : <RotateCw className="size-3" />}
          Check for updates
        </Button>
        {state.status === 'available' ? (
          <Button size="sm" variant="primary" onClick={() => void updates.install()}>
            Update now
          </Button>
        ) : null}
      </div>
      {line ? <p className="max-w-xs text-[11px] leading-relaxed text-muted-foreground">{line}</p> : null}
    </div>
  )
}
