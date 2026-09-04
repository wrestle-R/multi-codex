import { useId } from "react"
import type { Profile } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"

interface ConfirmDialogProps {
  profile: Profile
  busy: boolean
  error: string | null
  onCancel: () => void
  onConfirm: () => void
}

export function ConfirmDialog({ profile, busy, error, onCancel, onConfirm }: ConfirmDialogProps) {
  const titleId = useId()
  const dialogRef = useDialogFocus(onCancel, busy)

  return (
    <div className="dialog-layer" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && !busy && onCancel()}>
      <section ref={dialogRef} className="dialog confirm-dialog" role="alertdialog" aria-modal="true" aria-labelledby={titleId}>
        <span className="eyebrow danger-copy">Permanent action</span>
        <h2 id={titleId}>Delete {profile.name}?</h2>
        <p>This removes the saved credential and this profile's isolated Codex and VS Code data.</p>
        {error ? <div className="form-error" role="alert">{error}</div> : null}
        <div className="dialog-actions">
          <button className="button secondary" type="button" disabled={busy} onClick={onCancel}>Cancel</button>
          <button className="button destructive" type="button" disabled={busy} onClick={onConfirm}>{busy ? "Deleting" : "Delete account"}</button>
        </div>
      </section>
    </div>
  )
}
