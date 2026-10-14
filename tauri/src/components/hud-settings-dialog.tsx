import { useEffect, useId, useState } from "react"
import { defaultHudSettings, hudCall, type HudSettings, type HudView } from "../lib/hud"
import { useDialogFocus } from "./use-dialog-focus"

export function HudSettingsDialog({ onClose }: { onClose: () => void }) {
  const title = useId()
  const [settings, setSettings] = useState<HudSettings>(defaultHudSettings)
  const [monitors, setMonitors] = useState<string[]>([])
  const [loading, setLoading] = useState(true)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")
  const ref = useDialogFocus(onClose, busy)
  useEffect(() => {
    let active = true
    void Promise.all([hudCall<HudView>("hud_snapshot"), hudCall<string[]>("hud_monitors")]).then(([view, displays]) => {
      if (active) { setSettings(view.settings); setMonitors(displays); setLoading(false) }
    }).catch(error => { if (active) { setError(String(error)); setLoading(false) } })
    return () => { active = false }
  }, [])
  const update = <K extends keyof HudSettings>(key: K, value: HudSettings[K]) => setSettings(current => ({ ...current, [key]: value }))
  return <div className="dialog-backdrop"><section className="dialog" role="dialog" aria-modal="true" aria-labelledby={title} ref={ref}>
    <div className="dialog-header"><div><p className="eyebrow">Desktop companion</p><h2 id={title}>HUD settings</h2></div><button className="icon-button" aria-label="Close HUD settings" onClick={onClose}>×</button></div>
    <form onSubmit={event => {
      event.preventDefault(); setBusy(true); setError("")
      void hudCall("hud_save_settings", { settings }).then(onClose).catch(error => setError(String(error))).finally(() => setBusy(false))
    }}>
      <fieldset className="hud-settings-fields" disabled={loading || busy}>
        <label className="hud-check"><input type="checkbox" checked={settings.enabled} onChange={event => update("enabled", event.target.checked)} />Show the HUD</label>
        <label>Avatar size <output>{settings.size}px</output><div className="hud-presets">{[["Small", 40], ["Medium", 56], ["Large", 76]].map(([label, size]) => <button className="button secondary" type="button" key={label} aria-pressed={settings.size === size} onClick={() => update("size", Number(size))}>{label}</button>)}</div><input type="range" aria-label="Avatar size" min={32} max={100} value={settings.size} onChange={event => update("size", Number(event.target.value))} /></label>
        <label>Opacity <output>{Math.round(settings.opacity * 100)}%</output><input type="range" min={35} max={100} value={settings.opacity * 100} onChange={event => update("opacity", Number(event.target.value) / 100)} /></label>
        <label>Display<select value={settings.monitor ?? ""} onChange={event => setSettings(current => ({ ...current, monitor: event.target.value || null, x: null, y: null }))}><option value="">Primary display</option>{monitors.map(m => <option key={m} value={m}>{m}</option>)}</select></label>
        <button className="button secondary" type="button" onClick={() => setSettings(current => ({ ...current, x: null, y: null }))}>Reset position to top-right</button>
        <label className="hud-check"><input type="checkbox" checked={settings.onlyOpen} onChange={event => update("onlyOpen", event.target.checked)} />Only show accounts with open chats</label>
        <label className="hud-check"><input type="checkbox" checked={settings.animation} onChange={event => update("animation", event.target.checked)} />Subtle working animation</label>
        <label className="hud-check"><input type="checkbox" checked={settings.reducedMotion} onChange={event => update("reducedMotion", event.target.checked)} />Reduce motion</label>
        <label className="hud-check"><input type="checkbox" checked={settings.sound} onChange={event => update("sound", event.target.checked)} />Play a soft sound for new reminders</label>
        <p className="desktop-notice">Hover for usage and sessions. Click to return to Codex. The HUD hides during fullscreen. Show or hide it with Super + Shift + H or the tray.</p>
      </fieldset>
      {error && <p className="form-error" role="alert">{error}</p>}
      <div className="dialog-actions"><button type="button" className="button secondary" onClick={onClose} disabled={busy}>Cancel</button><button className="button primary" disabled={loading || busy}>{busy ? "Saving…" : "Save settings"}</button></div>
    </form>
  </section></div>
}
