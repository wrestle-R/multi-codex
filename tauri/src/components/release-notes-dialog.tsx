import { useId, useState } from "react"
import { acknowledgeAppRelease } from "../lib/desktop-api"
import { releaseNotesSince } from "../lib/release-notes"
import { useDialogFocus } from "./use-dialog-focus"

export function ReleaseNotesDialog({ version, previousVersion, platform, onClose }: { version: string; previousVersion?: string | null; platform?: string; onClose: () => void }) {
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
    <div className="release-notes-body">
      <p>{previousVersion ? `Updates since v${previousVersion}.` : "Release history included in this version."}</p>
      {releaseNotesSince(version, previousVersion).map(release => <section className="release-notes-entry" key={release.version} aria-label={`Release ${release.version}`}>
        <h3><span>v{release.version}</span>{release.title}</h3>
        <ul>{release.highlights.map(note => <li key={note}>{note}</li>)}</ul>
      </section>)}
      {platform === "windows" ? <p>Orbit is available on macOS and Linux.</p> : null}
    </div>
    {error ? <p className="form-error" role="alert">{error}</p> : null}
    <div className="dialog-actions">{error ? <button type="button" className="button secondary" onClick={onClose}>Close for now</button> : null}<button type="button" className="button primary" disabled={busy} onClick={() => void dismiss()}>{busy ? "Saving…" : "Got it"}</button></div>
  </section></div>
}
