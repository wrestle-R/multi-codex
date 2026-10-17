import { useEffect, useId, useState } from "react"
import { clearSharedChats, previewSharedChats } from "../lib/desktop-api"
import { formatStorage } from "../lib/formatters"
import type { ChatPreview, ChatScope } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"

export function SharedChatCleanupDialog({ scope, onClose, onCompleted }: {
  scope: ChatScope; onClose: () => void; onCompleted: (message: string) => void
}) {
  const title = useId()
  const description = useId()
  const [preview, setPreview] = useState<ChatPreview | null>(null)
  const [busy, setBusy] = useState(false)
  const [checking, setChecking] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [revision, setRevision] = useState(0)
  const dialog = useDialogFocus(onClose, busy)
  useEffect(() => {
    let active = true
    setChecking(true); setPreview(null); setError(null)
    void previewSharedChats(scope).then(value => { if (active) setPreview(value) })
      .catch(error => { if (active) setError(error instanceof Error ? error.message : String(error)) })
      .finally(() => { if (active) setChecking(false) })
    return () => { active = false }
  }, [scope, revision])
  async function clear() {
    if (!preview?.count || busy || checking) return
    setBusy(true); setError(null)
    try {
      const result = await clearSharedChats(preview.token)
      onCompleted(`Cleared ${result.deletedCount} shared chats for all accounts. Reclaimed ${formatStorage(result.reclaimedBytes)}. Original profile histories and backups are retained.`)
    } catch (error) { setPreview(null); setError(error instanceof Error ? error.message : String(error)) }
    finally { setBusy(false) }
  }
  return <div className="dialog-layer elevated-dialog-layer" role="presentation" onMouseDown={event => event.target === event.currentTarget && !busy && onClose()}>
    <section ref={dialog} className="dialog confirm-dialog" role="dialog" aria-modal="true" aria-labelledby={title} aria-describedby={description}>
      <span className="eyebrow">All saved accounts</span>
      <h2 id={title}>{scope === "all" ? "Clear shared chats" : "Clear older shared chats"}</h2>
      <p id={description}>This removes chats from the shared library for every account. Active chats and protected chat families stay available. Original private histories and verified backups are retained.</p>
      <div className="chat-cleanup-preview" role="status" aria-live="polite">
        {checking ? <p>Checking shared chats and active writers…</p> : preview ? <>
          <strong>{preview.count} shared chats selected</strong>
          <span>{preview.protectedCount} protected chats</span>
          <span>Space reclaimed depends on attachments still used by other chats.</span>
        </> : null}
        {busy ? <p>Clearing the selected library chats…</p> : null}
      </div>
      {error ? <div className="form-error" role="alert">{error}</div> : null}
      <div className="dialog-actions">
        <button className="button secondary" disabled={busy} onClick={onClose}>Close</button>
        <button className="button secondary" disabled={busy || checking} onClick={() => setRevision(value => value + 1)}>Check again</button>
        <button className="button primary danger-button" disabled={busy || checking || !preview?.count} onClick={() => void clear()}>{busy ? "Clearing…" : `Clear ${preview?.count ?? 0} shared chats`}</button>
      </div>
    </section>
  </div>
}
