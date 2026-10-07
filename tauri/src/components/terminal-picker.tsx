import { useEffect, useId, useRef, useState } from "react"
import type { TerminalId, TerminalOption } from "../lib/types"

export function TerminalPicker({ value, options, disabled, onChange }: {
  value: TerminalId
  options: TerminalOption[]
  disabled: boolean
  onChange: (value: TerminalId) => void
}) {
  const id = useId()
  const root = useRef<HTMLDivElement>(null)
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const [placement, setPlacement] = useState({ above: false, offset: 0, height: 240 })
  const search = useRef({ text: "", at: 0 })
  const installed = options.filter(option => option.id !== "automatic" && option.available)
  const choices = [{ id: "automatic" as const, label: "Automatic", available: true }, ...installed]
  const selected = options.find(option => option.id === value)
  const unavailable = value !== "automatic" && !installed.some(option => option.id === value)
  const label = value === "automatic" ? "Automatic" : selected?.label ?? value

  function show(index = Math.max(0, choices.findIndex(option => option.id === value))) {
    if (disabled) return
    const bounds = root.current?.getBoundingClientRect()
    const body = root.current?.closest(".settings-body")?.getBoundingClientRect()
    const trigger = root.current?.querySelector("button")
    const anchor = trigger?.getBoundingClientRect()
    if (bounds && body && trigger && anchor) {
      const below = body.bottom - anchor.bottom - 8
      const above = anchor.top - body.top - 8
      const height = Math.min(240, choices.length * 58)
      const useAbove = below < height + 48 && above > below
      setPlacement({ above: useAbove, offset: useAbove ? bounds.height - trigger.offsetTop + 8 : trigger.offsetTop + trigger.offsetHeight + 8, height: Math.max(32, Math.min(height, (useAbove ? above : below) - 48)) })
    }
    search.current = { text: "", at: 0 }
    setActive(index)
    setOpen(true)
  }
  function choose(index: number) {
    onChange(choices[index].id)
    setOpen(false)
  }
  useEffect(() => {
    if (!open) return
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false)
    }
    document.addEventListener("pointerdown", outside)
    const resize = () => setOpen(false)
    window.addEventListener("resize", resize)
    return () => { document.removeEventListener("pointerdown", outside); window.removeEventListener("resize", resize) }
  }, [open])
  useEffect(() => {
    if (open) root.current?.querySelector<HTMLElement>(`[id="${id}-option-${active}"]`)?.scrollIntoView?.({ block: "nearest" })
  }, [active, open, id])

  return <div ref={root} className="terminal-picker" onBlur={event => {
    if (!event.currentTarget.contains(event.relatedTarget)) setOpen(false)
  }}>
    <span id={`${id}-label`} className="settings-field-label">CLI terminal</span>
    <button type="button" role="combobox" className="terminal-picker-trigger" aria-labelledby={`${id}-label`} aria-describedby={`${id}-hint`} aria-haspopup="listbox" aria-expanded={open} aria-controls={open ? `${id}-list` : undefined} aria-activedescendant={open ? `${id}-option-${active}` : undefined} disabled={disabled} onClick={() => open ? setOpen(false) : show()} onKeyDown={event => {
      if (event.key === "Escape" && open) { event.preventDefault(); event.stopPropagation(); setOpen(false); return }
      if (event.key === "Tab") { setOpen(false); return }
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault()
        if (!open) show()
        else setActive(index => (index + (event.key === "ArrowDown" ? 1 : -1) + choices.length) % choices.length)
      } else if (event.key === "Home" || event.key === "End") {
        event.preventDefault()
        const index = event.key === "Home" ? 0 : choices.length - 1
        if (!open) show(index); else setActive(index)
      } else if (event.key === "Enter" || event.key === " ") {
        event.preventDefault()
        if (open) choose(active); else show()
      } else if (event.key.length === 1 && !event.altKey && !event.ctrlKey && !event.metaKey) {
        event.preventDefault()
        const now = Date.now()
        const text = (now - search.current.at < 700 ? search.current.text : "") + event.key.toLowerCase()
        const index = choices.findIndex(option => option.label.toLowerCase().startsWith(text))
        if (!open) show(index >= 0 ? index : 0)
        else if (index >= 0) setActive(index)
        search.current = { text, at: now }
      }
    }}>
      <span className="terminal-picker-icon" aria-hidden="true"><svg width="18" height="18" viewBox="0 0 24 24" fill="none"><rect x="3" y="4" width="18" height="16" rx="3" stroke="currentColor" strokeWidth="1.5" /><path d="m7 9 3 3-3 3m6 0h4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg></span>
      <span className="terminal-picker-value">{disabled ? "Detecting terminals…" : label}</span>
      <span className="terminal-picker-status">{unavailable ? "Unavailable" : value === "automatic" ? "Auto" : "Installed"}</span>
      <svg className="terminal-picker-chevron" width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg>
    </button>
    <p id={`${id}-hint`} className={`settings-help${unavailable ? " terminal-unavailable" : ""}`}>{unavailable ? `${label} is no longer available. Choose an installed terminal to launch the CLI.` : installed.length ? "Uses your selected account’s saved sign-in." : disabled ? "Checking this device for supported terminals." : "No supported terminal found. Install one, then reopen settings."}</p>
    {open ? <div className="terminal-picker-menu" style={{ top: placement.above ? "auto" : placement.offset, bottom: placement.above ? placement.offset : "auto" }}>
      <div className="terminal-menu-heading">On this device <span>{installed.length} installed</span></div>
      <ul id={`${id}-list`} role="listbox" aria-labelledby={`${id}-label`} style={{ maxHeight: placement.height }}>
        {choices.map((option, index) => <li key={option.id} id={`${id}-option-${index}`} role="option" aria-selected={value === option.id} className={index === active ? "terminal-option active" : "terminal-option"} onMouseDown={event => event.preventDefault()} onPointerMove={() => setActive(index)} onClick={() => choose(index)}>
          <span><strong>{option.label}</strong><small>{option.id === "automatic" ? "Choose an installed terminal automatically" : "Installed on this device"}</small></span>
          {value === option.id ? <svg width="17" height="17" viewBox="0 0 20 20" fill="none" aria-hidden="true"><path d="m4 10 4 4 8-8" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" /></svg> : null}
        </li>)}
      </ul>
    </div> : null}
  </div>
}
