import { useEffect, useState } from 'react'
import { Coffee, FolderSearch } from 'lucide-react'
import { Button, Caption, Segmented, inputCls } from '@/ui'
import { api, type JavaInfo } from '@/lib/api'
import { errorText } from '@/lib/bridge'
import { cn } from '@/lib/utils'

/** Memory in MB as a slider, with the computer's total as the end of the scale. */
export function MemorySlider({ value, totalMb, onChange }: { value: number; totalMb: number; onChange: (mb: number) => void }) {
  const max = Math.max(2048, Math.floor((totalMb * 0.75) / 512) * 512)
  const gb = (mb: number) => `${(mb / 1024).toFixed(mb % 1024 ? 1 : 0)} GB`
  const tight = value > totalMb * 0.6
  return (
    <div className="grid gap-2">
      <div className="flex items-center gap-4">
        <input
          type="range"
          min={1024}
          max={max}
          step={512}
          value={Math.min(value, max)}
          onChange={(e) => onChange(Number(e.target.value))}
          aria-label="Memory"
          className="h-1 grow cursor-pointer accent-[var(--primary)]"
        />
        <span className="w-16 text-right text-xs font-bold tabular-nums">{gb(value)}</span>
      </div>
      <p className={cn('text-[11px]', tight ? 'text-warning' : 'text-muted-foreground')}>
        {tight ? `That leaves little for Windows out of this computer's ${gb(totalMb)}.` : `Of this computer's ${gb(totalMb)}. Vanilla runs well on 2 to 4 GB; big mod packs want 6 to 8.`}
      </p>
    </div>
  )
}

/** Automatic (Mojang's runtime per version) or a Java the player picks. */
export function JavaPicker({ value, onChange, needs }: { value: string | null; onChange: (path: string | null) => void; needs?: number }) {
  const [info, setInfo] = useState<JavaInfo | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [draft, setDraft] = useState(value ?? '')
  useEffect(() => setDraft(value ?? ''), [value])
  useEffect(() => {
    setInfo(null)
    setError(null)
    if (!value) return
    let cancelled = false
    api
      .probeJava(value)
      .then((i) => !cancelled && setInfo(i))
      .catch((e) => !cancelled && setError(errorText(e)))
    return () => {
      cancelled = true
    }
  }, [value])

  return (
    <div className="grid gap-3">
      <Segmented
        value={value ? 'custom' : 'auto'}
        options={[
          { value: 'auto', label: 'Automatic' },
          { value: 'custom', label: 'My own Java' },
        ]}
        onChange={(v) => (v === 'auto' ? onChange(null) : void api.pickJava().then((p) => p && onChange(p)))}
      />
      {value ? (
        <div className="grid gap-2">
          <div className="flex gap-2">
            <input value={draft} onChange={(e) => setDraft(e.target.value)} onBlur={() => draft.trim() && draft !== value && onChange(draft.trim())} className={inputCls} spellCheck={false} />
            <Button onClick={() => void api.pickJava().then((p) => p && onChange(p))}>
              <FolderSearch className="size-3.5" /> Browse
            </Button>
          </div>
          {error ? (
            <p className="text-[11px] text-destructive">{error}</p>
          ) : info ? (
            <p className={cn('flex items-center gap-1.5 text-[11px]', needs && info.major < needs ? 'text-destructive' : 'text-muted-foreground')}>
              <Coffee className="size-3" /> Java {info.major} ({info.version}, {info.vendor || 'unknown vendor'}, {info.arch})
              {needs && info.major < needs ? ` - this version needs Java ${needs}.` : ''}
            </p>
          ) : (
            <Caption>Checking</Caption>
          )}
        </div>
      ) : (
        <p className="text-[11px] leading-relaxed text-muted-foreground">
          Each version gets the Java it was made for, downloaded from Mojang: Java 8 for old versions, 17, 21 or 25 for newer ones. Nothing to install.
        </p>
      )}
    </div>
  )
}

/** A number field for sizes (resolution). */
export function NumberField({ value, onChange, label, min = 0 }: { value: number; onChange: (n: number) => void; label: string; min?: number }) {
  const [draft, setDraft] = useState(String(value))
  useEffect(() => setDraft(String(value)), [value])
  return (
    <input
      aria-label={label}
      inputMode="numeric"
      value={draft}
      onChange={(e) => setDraft(e.target.value.replace(/\D/g, ''))}
      onBlur={() => {
        const n = Number(draft)
        if (Number.isFinite(n) && n >= min) onChange(n)
        else setDraft(String(value))
      }}
      className={cn(inputCls, 'w-24 text-center tabular-nums')}
    />
  )
}
