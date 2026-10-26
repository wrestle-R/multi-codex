import { useId, useState } from "react"
import { refreshIntervals, savePreferences, useAppPreferences } from "../lib/app-preferences"
import { useDialogFocus } from "./use-dialog-focus"

export function RefreshSettingsDialog({ onClose }: { onClose: () => void }) {
  const prefs = useAppPreferences()
  const [minutes, setMinutes] = useState(prefs.refreshMinutes)
  const [error, setError] = useState("")
  const title = useId()
  const ref = useDialogFocus(onClose, false)
  return <div className="dialog-layer"><section ref={ref} className="dialog preference-dialog refresh-settings-dialog" role="dialog" aria-modal="true" aria-labelledby={title}>
    <header className="dialog-header"><div><p className="eyebrow">Settings</p><h2 id={title}>Refresh</h2></div><button type="button" className="icon-button" aria-label="Close refresh settings" onClick={onClose}>×</button></header>
    <form onSubmit={event => { event.preventDefault(); try { savePreferences({ ...prefs, refreshMinutes: minutes }); onClose() } catch { setError("Could not save your refresh preference. Check available storage and try again.") } }}>
      <div className="preference-body">
        <label className="preference-label">Check usage and app updates every<select value={minutes} onChange={event => setMinutes(Number(event.target.value))}>{refreshIntervals.map(value => <option key={value} value={value}>{value} {value === 1 ? "minute" : "minutes"}{value === 10 ? " (default)" : ""}</option>)}</select></label>
        <p className="settings-help">Faster checks use more battery and network, and may hit service rate limits. 10 minutes is recommended.</p>
        <p className="settings-help">Checks continue in the tray. Refresh all checks immediately.</p>
        {error && <p className="form-error" role="alert">{error}</p>}
      </div><div className="dialog-actions"><button type="button" className="button secondary" onClick={onClose}>Cancel</button><button className="button primary">Save settings</button></div>
    </form>
  </section></div>
}
