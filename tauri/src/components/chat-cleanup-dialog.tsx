import { useEffect, useId, useState } from "react"
import { clearProfileChats, previewProfileChats } from "../lib/desktop-api"
import { formatStorage } from "../lib/formatters"
import type { ChatCleanupResult, ChatPreview, ChatScope, ProfileStorageUsage } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"

interface ChatCleanupDialogProps {
  profile: ProfileStorageUsage
  scope: ChatScope
  onClose: () => void
  onDeleted: (result: ChatCleanupResult) => void
}

export function ChatCleanupDialog({ profile, scope, onClose, onDeleted }: ChatCleanupDialogProps) {
  const titleId = useId()
  const descriptionId = useId()
  const [preview, setPreview] = useState<ChatPreview | null>(null)
  const [checking, setChecking] = useState(true)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [revision, setRevision] = useState(0)
  const dialogRef = useDialogFocus(onClose, busy)

  useEffect(() => {
    let active = true
    setChecking(true)
    setPreview(null)
    setError(null)
    void previewProfileChats(profile.id, scope).then(value => {
      if (active) setPreview(value)
    }).catch(error => {
      if (active) setError(error instanceof Error ? error.message : String(error))
    }).finally(() => {
      if (active) setChecking(false)
    })
    return () => { active = false }
  }, [profile.id, scope, revision])

  async function clearChats() {
    if (!preview || !preview.count || busy || checking) return
    setBusy(true)
    setError(null)
    try {
      const result = await clearProfileChats(profile.id, preview.token)
      onDeleted(result)
    } catch (error) {
      setPreview(null)
      setError(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="dialog-layer elevated-dialog-layer" role="presentation" onMouseDown={event => event.target === event.currentTarget && !busy && onClose()}>
      <section ref={dialogRef} className="dialog confirm-dialog" role="alertdialog" aria-modal="true" aria-labelledby={titleId} aria-describedby={descriptionId}>
        <span className="eyebrow">Local chat history</span>
        <h2 id={titleId}>{scope === "all" ? "Clear all chats" : "Clear chats older than 30 days"} from {profile.name}?</h2>
        <p id={descriptionId}>{scope === "all" ? "This permanently deletes this profile’s saved local chats, including archived chats and subagent chats." : "This permanently deletes saved local chats last active more than 30 days ago, including archived chats. Chats with newer subagent activity are kept."} Deleted chats cannot be resumed. Your account, settings, skills, plugins, and extensions are kept. Cloud conversations are not deleted.</p>
        <div className="chat-cleanup-preview" role="status" aria-live="polite">
          {checking ? <p>Checking local chat history…</p> : preview ? <>
            <strong>{preview.count} {preview.count === 1 ? "chat" : "chats"} selected</strong>
            <span>{formatStorage(preview.transcriptBytes)} of transcripts · history records are also removed</span>
            {preview.count === 0 ? <p>No chats match this cleanup.</p> : null}
            {preview.protectedCount > 0 ? <p>{preview.protectedCount} older {preview.protectedCount === 1 ? "chat kept" : "chats kept"} to preserve newer subagent chats.</p> : null}
          </> : null}
        </div>
        {error ? <div className="form-error" role="alert">{error}</div> : null}
        <div className="dialog-actions">
          <button className="button secondary" type="button" disabled={busy} onClick={onClose}>Cancel</button>
          {error ? <button className="button secondary" type="button" disabled={checking || busy} onClick={() => setRevision(value => value + 1)}>Check again</button> : null}
          <button className="button destructive" type="button" disabled={checking || busy || !preview?.count} onClick={() => void clearChats()}>{busy ? "Deleting chats…" : "Delete selected chats"}</button>
        </div>
      </section>
    </div>
  )
}
