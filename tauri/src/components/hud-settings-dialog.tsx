import { useEffect, useId, useState } from "react"
import { defaultHudSettings, hudCall, type HudSettings, type HudView } from "../lib/hud"
import { AccountAvatar } from "./account-avatar"
import { avatarTemplates } from "../lib/avatar-templates"
import { savePreferences, useAppPreferences } from "../lib/app-preferences"
import { useDialogFocus } from "./use-dialog-focus"

export function HudSettingsDialog({ onClose }: { onClose: () => void }) {
  const title = useId()
  const prefs = useAppPreferences()
  const [seasonal, setSeasonal] = useState(prefs.seasonalAvatars)
  const [settings, setSettings] = useState<HudSettings>(defaultHudSettings)
  const [monitors, setMonitors] = useState<string[]>([])
  const [loading, setLoading] = useState(true)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")
  const [platformError, setPlatformError] = useState<string | null>(null)
  const ref = useDialogFocus(onClose, busy)
  useEffect(() => {
    let active = true
    void Promise.all([hudCall<HudView>("hud_snapshot"), hudCall<string[]>("hud_monitors")]).then(([view, displays]) => {
      if (active) { setSettings(view.settings); setMonitors(displays); setPlatformError(view.platformError ?? null); setLoading(false) }
    }).catch(error => { if (active) { setError(String(error)); setLoading(false) } })
    return () => { active = false }
  }, [])
  const update = <K extends keyof HudSettings>(key: K, value: HudSettings[K]) => setSettings(current => ({ ...current, [key]: value }))
  return <div className="dialog-layer"><section className="dialog orbit-settings-dialog" role="dialog" aria-modal="true" aria-labelledby={title} ref={ref}>
    <div className="dialog-header"><div><p className="eyebrow">Orbiters</p><h2 id={title}>Orbit settings</h2></div><button className="icon-button" aria-label="Close Orbit settings" disabled={busy} onClick={onClose}>×</button></div>
    <form onSubmit={event => {
      event.preventDefault(); setBusy(true); setError("")
      void hudCall("hud_save_settings", { settings }).then(() => { savePreferences({ ...prefs, seasonalAvatars: seasonal }); onClose() }).catch(error => setError(String(error))).finally(() => setBusy(false))
    }}>
      <div className="orbit-settings-body"><fieldset className="hud-settings-fields" disabled={loading || busy}>
        <label className="hud-check"><input type="checkbox" checked={settings.enabled} onChange={event => update("enabled", event.target.checked)} />Enable Orbit</label>
        <p className="settings-help">Hide Orbiters without stopping your chats.</p>
        {platformError && <p className="form-error" role="status">{platformError}</p>}
        <section className="settings-section seasonal-settings" aria-label="Seasonal costumes">
          <div className="settings-section-heading"><h3>Avatars</h3></div>
          <div className="avatar-template-preview" aria-label="Six avatar templates">{avatarTemplates.map(template => <div key={template.name}><AccountAvatar id={`template-${template.name}`} identity={template} size={64} /><span>{template.name}</span></div>)}</div>
          <label className="hud-check"><input type="checkbox" checked={seasonal} onChange={event => setSeasonal(event.target.checked)} />Festival clothing</label>
          <p className="settings-help">Outfits change automatically during festivals. Same colour, same character.</p>
        </section>
        <label>Orbiter size <output>{settings.size}px</output><div className="hud-presets">{[["Small", 40], ["Medium", 56], ["Large", 76]].map(([label, size]) => <button className="button secondary" type="button" key={label} aria-pressed={settings.size === size} onClick={() => update("size", Number(size))}>{label}</button>)}</div><input type="range" aria-label="Orbiter size" min={32} max={100} value={settings.size} onChange={event => update("size", Number(event.target.value))} /></label>
        <label>Opacity <output>{Math.round(settings.opacity * 100)}%</output><input type="range" min={35} max={100} value={settings.opacity * 100} onChange={event => update("opacity", Number(event.target.value) / 100)} /></label>
        <label>Display<select value={settings.monitor ?? ""} onChange={event => setSettings(current => ({ ...current, monitor: event.target.value || null, x: null, y: null }))}><option value="">Primary display</option>{monitors.map(m => <option key={m} value={m}>{m}</option>)}</select></label>
        <button className="button secondary" type="button" onClick={() => setSettings(current => ({ ...current, x: null, y: null }))}>Reset position to top-right</button>
        <label className="hud-check"><input type="checkbox" checked={settings.onlyOpen} onChange={event => update("onlyOpen", event.target.checked)} />Only show accounts with chat shortcuts</label>
        <label className="hud-check"><input type="checkbox" checked={settings.animation} onChange={event => update("animation", event.target.checked)} />Pulse working status dot</label>
        <label className="hud-check"><input type="checkbox" checked={settings.reducedMotion} onChange={event => update("reducedMotion", event.target.checked)} />Reduce motion</label>
        <label className="hud-check"><input type="checkbox" checked={settings.sound} onChange={event => update("sound", event.target.checked)} />Play a soft sound for new reminders</label>
        <p className="desktop-notice">Hover for usage and sessions. Click to return to Codex. Orbit hides during fullscreen. Show or hide it with ⌘ / Super + Shift + H or the tray.</p>
      </fieldset></div>
      {error && <p className="form-error" role="alert">{error}</p>}
      <div className="dialog-actions"><button type="button" className="button secondary" onClick={onClose} disabled={busy}>Cancel</button><button className="button primary" disabled={loading || busy}>{busy ? "Saving…" : "Save settings"}</button></div>
    </form>
  </section></div>
}
