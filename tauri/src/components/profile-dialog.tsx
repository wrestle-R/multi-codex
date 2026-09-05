import { Cancel01Icon } from "@hugeicons/core-free-icons"
import { HugeiconsIcon } from "@hugeicons/react"
import { useId, useState } from "react"
import type { FormEvent } from "react"
import type { Profile, ProfileDetails } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"

interface ProfileDialogProps {
  profile?: Profile | null
  busy: boolean
  error: string | null
  onClose: () => void
  onSave: (name: string, authJson: string | undefined, details: ProfileDetails) => Promise<void>
  onImportCurrent: (name: string, details: ProfileDetails) => Promise<void>
}

export function ProfileDialog({
  profile,
  busy,
  error,
  onClose,
  onSave,
  onImportCurrent,
}: ProfileDialogProps) {
  const titleId = useId()
  const [mode, setMode] = useState<"paste" | "current">("paste")
  const [name, setName] = useState(profile?.name ?? "")
  const [authJson, setAuthJson] = useState("")
  const [notes, setNotes] = useState(profile?.notes ?? "")
  const dialogRef = useDialogFocus(onClose, busy)

  const canSubmit = name.trim().length > 0 && (Boolean(profile) || mode === "current" || authJson.trim().length > 0)

  async function submit(event: FormEvent) {
    event.preventDefault()
    const details: ProfileDetails = {
      notes: notes.trim() || undefined,
    }
    if (profile || mode === "paste") await onSave(name, authJson.trim() || undefined, details)
    else await onImportCurrent(name, details)
  }

  return (
    <div className="dialog-layer" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && !busy && onClose()}>
      <section ref={dialogRef} className="dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <div className="dialog-header">
          <div>
            <span className="eyebrow">Account profile</span>
            <h2 id={titleId}>{profile ? "Edit account" : "Add account"}</h2>
          </div>
          <button className="dialog-close" type="button" aria-label="Close" title="Close" disabled={busy} onClick={onClose}>
            <HugeiconsIcon icon={Cancel01Icon} size={20} strokeWidth={1.8} />
          </button>
        </div>

        {!profile ? (
          <div className="segmented-control" aria-label="Account source">
            <button type="button" className={mode === "paste" ? "active" : ""} onClick={() => setMode("paste")}>Paste JSON</button>
            <button type="button" className={mode === "current" ? "active" : ""} onClick={() => setMode("current")}>Import current</button>
          </div>
        ) : null}

        <form onSubmit={submit}>
          <label>
            <span>Profile name</span>
            <input autoFocus value={name} maxLength={64} placeholder="Personal" onChange={(event) => setName(event.currentTarget.value)} />
          </label>

          {mode === "paste" ? (
            <label>
              <span>{profile ? "Replace auth JSON (optional)" : "Auth JSON"}</span>
              <textarea
                value={authJson}
                placeholder={'{"auth_mode":"chatgpt", ...}'}
                spellCheck={false}
                onChange={(event) => setAuthJson(event.currentTarget.value)}
              />
            </label>
          ) : (
            <div className="notice">Reads your current Codex login and saves a protected copy. The original file is never changed.</div>
          )}

          <label>
            <span>Notes <small>Optional</small></span>
            <textarea
              className="notes-input"
              value={notes}
              maxLength={500}
              placeholder="Anything useful about this account"
              onChange={(event) => setNotes(event.currentTarget.value)}
            />
          </label>

          {error ? <div className="form-error" role="alert">{error}</div> : null}

          <div className="dialog-actions">
            <button className="button secondary" type="button" disabled={busy} onClick={onClose}>Cancel</button>
            <button className="button primary" type="submit" disabled={!canSubmit || busy}>{busy ? "Saving" : profile ? "Save changes" : "Add account"}</button>
          </div>
        </form>
      </section>
    </div>
  )
}
