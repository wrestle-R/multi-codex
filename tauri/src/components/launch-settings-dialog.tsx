import { useEffect, useId, useState } from "react"
import { getExecutableSettings, saveExecutableSettings } from "../lib/desktop-api"
import type { ExecutableSettings } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"

export function LaunchSettingsDialog({ onClose, isMac }: { onClose: () => void; isMac: boolean }) {
  const titleId = useId()
  const [settings, setSettings] = useState<ExecutableSettings>({ codePath: null, codexPath: null, globalCodexHome: null })
  const [loading, setLoading] = useState(true)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const dialogRef = useDialogFocus(onClose, busy)
  useEffect(() => {
    let active = true
    void getExecutableSettings().then(value => { if (active) setSettings(value) }).catch(error => { if (active) setError(String(error)) }).finally(() => { if (active) setLoading(false) })
    return () => { active = false }
  }, [])
  async function save(event: React.FormEvent) {
    event.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await saveExecutableSettings(settings)
      onClose()
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error))
    } finally { setBusy(false) }
  }
  return (
    <div className="dialog-layer" role="presentation" onMouseDown={event => event.target === event.currentTarget && !busy && onClose()}>
      <section ref={dialogRef} className="dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <span className="eyebrow">Launch settings</span>
        <h2 id={titleId}>Folders and tools</h2>
        <p>Leave paths empty for automatic discovery. Use full paths without quotes or command arguments.</p>
        <form onSubmit={event => void save(event)} className="launch-settings-form">
          <label>Preferred folder<input value={settings.preferredWorkspace ?? ""} disabled={loading || busy} placeholder="Default workspace folder" onChange={event => setSettings(value => ({ ...value, preferredWorkspace: event.target.value || null }))} /></label>
          <p className="desktop-notice">Start the folder picker here. You can choose a different project for each launch.</p>
          {!isMac ? <label className="desktop-picker-setting"><input type="checkbox" checked={!(settings.hideDesktopPicker ?? false)} disabled={loading || busy} onChange={event => setSettings(value => ({ ...value, hideDesktopPicker: !event.target.checked }))} /> Show desktop picker before launching</label> : null}
          <label>VS Code executable<input value={settings.codePath ?? ""} disabled={loading || busy} placeholder="Automatic discovery" onChange={event => setSettings(value => ({ ...value, codePath: event.target.value || null }))} /></label>
          <label>Codex executable<input value={settings.codexPath ?? ""} disabled={loading || busy} placeholder="Automatic discovery" onChange={event => setSettings(value => ({ ...value, codexPath: event.target.value || null }))} /></label>
          <label>Global Codex home<input value={settings.globalCodexHome ?? ""} disabled={loading || busy} placeholder="CODEX_HOME or ~/.codex" onChange={event => setSettings(value => ({ ...value, globalCodexHome: event.target.value || null }))} /></label>
          <p className="desktop-notice">Executable changes apply to the next launch. Restart Multi Codex after changing the global home. An inherited CODEX_HOME takes precedence. Existing isolated profiles stay in their current location.</p>
          {error ? <p className="form-error" role="alert">{error}</p> : null}
          <div className="dialog-actions"><button type="button" className="button secondary" disabled={busy} onClick={onClose}>Cancel</button><button type="submit" className="button primary" disabled={loading || busy}>{busy ? "Saving…" : "Save paths"}</button></div>
        </form>
      </section>
    </div>
  )
}
