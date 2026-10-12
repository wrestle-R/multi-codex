import { useEffect, useId, useState } from "react"
import { getSharedStorageQueue, setSharedStorageEnabled, previewSharedGenerations, clearSharedGenerations, type GenerationCleanup } from "../lib/desktop-api"
import { formatStorage } from "../lib/formatters"
import type { Profile, SharedStorageQueue } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"

export function SharedStorageQueueDialog({ profiles, onClose }: { profiles: Profile[]; onClose: () => void }) {
  const title = useId()
  const [queue, setQueue] = useState<SharedStorageQueue | null>(null)
  const [packagePreview, setPackagePreview] = useState<GenerationCleanup | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const dialog = useDialogFocus(onClose, busy)
  useEffect(() => {
    let active = true
    const refresh = () => void getSharedStorageQueue().then(value => { if (active) setQueue(value) })
      .catch(error => { if (active) setError(error instanceof Error ? error.message : String(error)) })
    refresh()
    const timer = window.setInterval(refresh, 5000)
    return () => { active = false; window.clearInterval(timer) }
  }, [])
  async function clearPackages() {
    if (busy) return
    setBusy(true); setError(null); setNotice(null)
    try {
      if (!packagePreview) { setPackagePreview(await previewSharedGenerations()); return }
      const bytes = await clearSharedGenerations(packagePreview)
      setPackagePreview(null); setNotice(`Cleared ${formatStorage(bytes)} of unused package versions. Referenced catalogs, the current and previous versions, and backups are kept.`)
    } catch (error) { setPackagePreview(null); setError(error instanceof Error ? error.message : String(error)) }
    finally { setBusy(false) }
  }
  async function change(enabled: boolean) {
    if (busy) return
    setBusy(true); setError(null)
    try { setQueue(await setSharedStorageEnabled(enabled)) }
    catch (error) { setError(error instanceof Error ? error.message : String(error)) }
    finally { setBusy(false) }
  }
  return <div className="dialog-layer elevated-dialog-layer" role="presentation" onMouseDown={event => event.target === event.currentTarget && !busy && onClose()}>
    <section ref={dialog} className="dialog confirm-dialog" role="dialog" aria-modal="true" aria-labelledby={title}>
      <span className="eyebrow">Shared VS Code storage</span>
      <h2 id={title}>Set up all accounts</h2>
      <p>Each closed account gets a verified backup, shared Codex packages, and complete chat imports. Accounts in use wait until their sessions close. New accounts join this setup automatically. Logins, private settings, Copilot, original folders, and backups are kept.</p>
      <div className="chat-cleanup-preview" role="status" aria-live="polite">
        {!queue ? <p>Reading migration status…</p> : <>
          <strong>{queue.enabled ? "Automatic setup enabled" : "Automatic setup paused"}</strong>
          {profiles.map(profile => {
            const status = queue.profiles[profile.id]
            return <span key={profile.id}>{profile.name}: {status?.state === "ready" ? "ready" : status?.state === "needsAttention" ? "needs attention" : status?.state === "deferred" ? "waiting for sessions to close" : queue.enabled ? "queued" : "original layout"}{status?.reason ? ` · ${status.reason}` : ""}</span>
          })}
        </>}
      </div>
      {packagePreview ? <p role="status">{packagePreview.generations.length} unused package versions · {formatStorage(packagePreview.bytes)}. Close all account sessions before confirming cleanup. Every referenced catalog and the previous package version are kept.</p> : null}
      {notice ? <p role="status">{notice}</p> : null}
      {error ? <div className="form-error" role="alert">{error}</div> : null}
      <div className="dialog-actions">
        <button className="button secondary" disabled={busy} onClick={onClose}>Close</button>
        <button className="button secondary" disabled={busy || packagePreview?.generations.length === 0} onClick={() => void clearPackages()}>{packagePreview ? "Confirm unused package cleanup" : "Preview unused package versions"}</button>
        {queue?.enabled ? <button className="button secondary" disabled={busy} onClick={() => void change(false)}>Pause queued setup</button> : null}
        <button className="button primary" disabled={busy || !queue} onClick={() => void change(true)}>{busy ? "Saving…" : queue?.enabled ? "Retry accounts needing attention" : "Back up and queue all accounts"}</button>
      </div>
    </section>
  </div>
}
