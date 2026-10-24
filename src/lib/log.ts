import { call, errorText, isTauri } from '@/lib/bridge'

/**
 * The window's side of the client log (`logging.rs`): uncaught errors and
 * failed promises are written to it, and errors reach kryo.to's report
 * queue the same way the native side's do.
 */

export function logError(scope: string, e: unknown) {
  const message = errorText(e)
  if (!isTauri()) {
    console.error(`[${scope}]`, message)
    return
  }
  void call('log_write', { level: 'error', scope, message }).catch(() => {})
}

/** Written to the log on this PC only: expected trouble that has a fallback. */
export function logWarn(scope: string, message: string) {
  if (!isTauri()) {
    console.warn(`[${scope}]`, message)
    return
  }
  void call('log_write', { level: 'warn', scope, message }).catch(() => {})
}

export function logInfo(scope: string, message: string) {
  if (isTauri()) void call('log_write', { level: 'info', scope, message }).catch(() => {})
}

export function installErrorLogging(windowLabel: string) {
  const scope = `ui:${windowLabel}`
  window.addEventListener('error', (e) => {
    const where = e.filename ? ` (${e.filename.split('/').pop()}:${e.lineno}:${e.colno})` : ''
    logError(scope, `${e.message}${where}`)
  })
  window.addEventListener('unhandledrejection', (e) => {
    const reason = e.reason instanceof Error ? `${e.reason.message}\n${e.reason.stack ?? ''}` : errorText(e.reason)
    logError(scope, `unhandled: ${reason}`)
  })
}
