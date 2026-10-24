import { useEffect, useState, type ReactNode } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { KryoMark } from '@/ui/ascii/KryoMark'
import { closeWindow, isTauri, isWindowsHost, minimizeWindow, toggleMaximize, useWindowState } from '@/lib/window'
import { cn } from '@/lib/utils'

/**
 * The window's top strip, as Kryoto Desktop draws it: the mark, the app's
 * name, a drag area, whatever the shell puts on the right (the profile), and
 * the window buttons. Double-click the empty part to maximize.
 */
export function TitleBar({ right }: { right?: ReactNode }) {
  return (
    <header className="relative z-50 flex h-9 shrink-0 select-none items-stretch border-b border-border bg-background">
      <div data-maximize className="drag flex items-center gap-3 pl-3.5 pr-2">
        <KryoMark className="pointer-events-none h-3.5" />
        <span className="pointer-events-none text-[10px] uppercase tracking-[0.25em] text-muted-foreground">Minecraft Launcher</span>
      </div>
      <div data-maximize className="drag grow" />
      <div className="no-drag flex items-center gap-1.5 pr-2">{right}</div>
      {isTauri() ? <WindowControls /> : null}
    </header>
  )
}

/** Minimize, maximize or restore, and close, in Windows 11's own glyphs (Desktop's). */
function WindowControls() {
  const { maximized, fullscreen } = useWindowState()
  const active = useWindowFocus()
  const glyphs = isWindowsHost()
  return (
    <div className={cn('no-drag flex items-stretch', !active && 'opacity-60')}>
      <WindowButton label="Minimize" onClick={() => void minimizeWindow()} glyph={glyphs ? '' : undefined}>
        <path d="M0 5.5h10" />
      </WindowButton>
      {fullscreen ? null : (
        <WindowButton label={maximized ? 'Restore down' : 'Maximize'} onClick={() => void toggleMaximize()} glyph={glyphs ? (maximized ? '' : '') : undefined}>
          {maximized ? (
            <>
              <path d="M2.5 2.5V0.5h7v7h-2" />
              <rect x="0.5" y="2.5" width="7" height="7" />
            </>
          ) : (
            <rect x="0.5" y="0.5" width="9" height="9" />
          )}
        </WindowButton>
      )}
      <WindowButton label="Close" onClick={() => void closeWindow()} danger glyph={glyphs ? '' : undefined}>
        <path d="M0.5 0.5l9 9M9.5 0.5l-9 9" />
      </WindowButton>
    </div>
  )
}

function useWindowFocus() {
  const [active, setActive] = useState(() => (typeof document === 'undefined' ? true : document.hasFocus()))
  useEffect(() => {
    if (!isTauri()) return
    let stop: (() => void) | undefined
    let cancelled = false
    void getCurrentWindow()
      .onFocusChanged(({ payload }) => setActive(payload))
      .then((fn) => (cancelled ? fn() : (stop = fn)))
    return () => {
      cancelled = true
      stop?.()
    }
  }, [])
  return active
}

function WindowButton({ label, onClick, danger = false, glyph, children }: { label: string; onClick: () => void; danger?: boolean; glyph?: string; children: ReactNode }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onMouseDown={(e) => e.stopPropagation()}
      onClick={onClick}
      className={cn(
        'kryo-square grid w-[46px] place-items-center text-foreground/90 transition-colors duration-75',
        danger ? 'hover:bg-[#c42b1c] hover:text-white active:bg-[#c42b1c]/90 active:text-white/80' : 'hover:bg-foreground/[0.06] active:bg-foreground/[0.04] active:text-foreground/60',
      )}
    >
      {glyph ? (
        <span className="kryo-caption" aria-hidden>
          {glyph}
        </span>
      ) : (
        <svg viewBox="0 0 10 10" className="size-2.5 overflow-visible [&_*]:[vector-effect:non-scaling-stroke]" fill="none" stroke="currentColor" strokeWidth="1" aria-hidden>
          {children}
        </svg>
      )}
    </button>
  )
}
