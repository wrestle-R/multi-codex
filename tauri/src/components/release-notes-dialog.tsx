import { useId, useState } from "react"
import { acknowledgeAppRelease } from "../lib/desktop-api"
import { useDialogFocus } from "./use-dialog-focus"

export function ReleaseNotesDialog({ version, platform, onClose }: { version: string; platform?: string; onClose: () => void }) {
  const title = useId()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")
  async function dismiss() {
    setBusy(true)
    setError("")
    try { await acknowledgeAppRelease(); onClose() }
    catch { setError("Could not save your preference. Try again, or close for now.") }
    finally { setBusy(false) }
  }
  const ref = useDialogFocus(() => { void dismiss() }, busy)
  return <div className="dialog-layer"><section className="dialog release-notes-dialog" role="dialog" aria-modal="true" aria-labelledby={title} ref={ref}>
    <header className="settings-header"><div><span className="eyebrow">v{version}</span><h2 id={title}>What’s new</h2></div><button type="button" className="dialog-close" aria-label="Close release notes" disabled={busy} onClick={() => void dismiss()}>×</button></header>
    <ul>
      {platform !== "windows" ? <li><strong>Orbit.</strong> Your Orbiters show live Codex Desktop sessions. Hover for usage; click to return to a chat.</li> : null}
      {platform !== "windows" ? <li><strong>Your choice.</strong> Turn Orbit off in Settings. Adjust Orbiter size, display, motion and sound in Orbit preferences.</li> : null}
      <li><strong>Shared chats.</strong> More reliable history, model settings, archive actions and account handoff.</li>
    </ul>
    {error ? <p className="form-error" role="alert">{error}</p> : null}
    <div className="dialog-actions">{error ? <button type="button" className="button secondary" onClick={onClose}>Close for now</button> : null}<button type="button" className="button primary" disabled={busy} onClick={() => void dismiss()}>{busy ? "Saving…" : "Got it"}</button></div>
  </section></div>
}
