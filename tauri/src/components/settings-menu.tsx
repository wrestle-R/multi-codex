import { useEffect, useId, useRef, useState } from "react"
import { ThemeChoices } from "./theme-picker"
import { useDialogFocus } from "./use-dialog-focus"
import type { ColorTheme } from "../lib/themes"

export function SettingsMenu({ colorTheme, onTheme, onOrbit, onLaunch, onStorage, storage, disabled }: {
  colorTheme: ColorTheme; onTheme: (theme: ColorTheme) => void; onOrbit?: () => void
  onLaunch: () => void; onStorage: () => void; storage: string; disabled: boolean
}) {
  const [open, setOpen] = useState(false)
  const id = useId()
  const root = useRef<HTMLDivElement>(null)
  const dialog = useDialogFocus(() => setOpen(false), false, open)
  useEffect(() => {
    if (!open) return
    function outside(event: PointerEvent) {
      if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false)
    }
    document.addEventListener("pointerdown", outside)
    return () => document.removeEventListener("pointerdown", outside)
  }, [open])
  useEffect(() => { if (disabled) setOpen(false) }, [disabled])
  function navigate(action: () => void) { setOpen(false); action() }
  return <div className="settings-menu" ref={root}>
    <button className="button secondary" type="button" aria-expanded={open} aria-haspopup="dialog" aria-controls={open ? id : undefined} disabled={disabled} onClick={() => setOpen(value => !value)}>Settings <span aria-hidden="true">⌄</span></button>
    {open && <section className="settings-popover" role="dialog" aria-label="Quick settings" id={id} ref={dialog}>
      <div className="settings-popover-heading"><strong>Make yourself at home</strong><button type="button" className="icon-button" aria-label="Close quick settings" onClick={() => setOpen(false)}>×</button></div>
      <div className="settings-appearance"><div className="settings-group-label"><strong>Appearance</strong><span>Light or dark, your colours stay.</span></div><ThemeChoices value={colorTheme} onChange={onTheme} /></div>
      <div className="settings-destinations">
        {onOrbit && <button type="button" onClick={() => navigate(onOrbit)}><span><strong>Orbit</strong><small>Companions, display and reminders</small></span><span aria-hidden="true">↗</span></button>}
        <button type="button" onClick={() => navigate(onLaunch)}><span><strong>Launch settings</strong><small>Apps, terminals and workspace defaults</small></span><span aria-hidden="true">↗</span></button>
        <button type="button" onClick={() => navigate(onStorage)}><span><strong>Storage <em>{storage}</em></strong><small>See what’s using space</small></span><span aria-hidden="true">↗</span></button>
      </div>
    </section>}
  </div>
}
