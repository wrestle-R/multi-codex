import { useEffect, useId, useRef, useState } from "react"
import { colorThemes, type ColorTheme } from "../lib/themes"

export function ThemePicker({ value, onChange }: { value: ColorTheme; onChange: (theme: ColorTheme) => void }) {
  const [open, setOpen] = useState(false)
  const id = useId()
  const root = useRef<HTMLDivElement>(null)
  const trigger = useRef<HTMLButtonElement>(null)
  function close() { setOpen(false); trigger.current?.focus() }
  useEffect(() => {
    if (!open) return
    root.current?.querySelector<HTMLButtonElement>('[aria-pressed="true"]')?.focus()
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false)
    }
    document.addEventListener("pointerdown", outside)
    return () => document.removeEventListener("pointerdown", outside)
  }, [open])
  return (
    <div ref={root} className="theme-picker" onKeyDown={event => {
      if (event.key === "Escape" && open) { event.stopPropagation(); close() }
    }} onBlur={event => {
      if (!event.currentTarget.contains(event.relatedTarget)) setOpen(false)
    }}>
      <button ref={trigger} type="button" className="button secondary theme-picker-trigger" aria-label={`Color theme: ${colorThemes.find(theme => theme.id === value)?.name}`} aria-haspopup="dialog" aria-expanded={open} aria-controls={open ? id : undefined} onClick={() => setOpen(current => !current)}>
        {colorThemes.find(theme => theme.id === value)?.name}
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg>
      </button>
      {open ? <section id={id} className="theme-popover" role="dialog" aria-label="Choose a theme">
        <div className="theme-popover-heading"><strong>Choose a theme</strong><span>Works in light & dark</span></div>
        <div className="theme-options">
          {colorThemes.map(theme => <button key={theme.id} type="button" className={`theme-option theme-preview-${theme.id}`} aria-pressed={value === theme.id} onClick={() => { onChange(theme.id); close() }}>
            <span className="theme-preview" aria-hidden="true"><i /><i /><i /></span>
            <span className="theme-option-name">{theme.name}<span aria-hidden="true">{value === theme.id ? "✓" : ""}</span></span>
          </button>)}
        </div>
      </section> : null}
    </div>
  )
}
