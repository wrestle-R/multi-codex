import { useId } from "react"
import type { ProfileStorageUsage } from "../lib/types"
import { formatStorage } from "../lib/formatters"
import { useDialogFocus } from "./use-dialog-focus"

interface CacheConfirmDialogProps {
  profile: ProfileStorageUsage
  busy: boolean
  error: string | null
  onCancel: () => void
  onConfirm: () => void
}

export function CacheConfirmDialog({ profile, busy, error, onCancel, onConfirm }: CacheConfirmDialogProps) {
  const titleId = useId()
  const dialogRef = useDialogFocus(onCancel, busy)
  return (
    <div className="dialog-layer elevated-dialog-layer" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && !busy && onCancel()}>
      <section ref={dialogRef} className="dialog confirm-dialog" role="alertdialog" aria-modal="true" aria-labelledby={titleId}>
        <span className="eyebrow">Safe cleanup</span>
        <h2 id={titleId}>Clear {formatStorage(profile.reclaimableBytes)} from {profile.name}?</h2>
        <p>Only disposable caches are removed. Credentials, conversations, settings, skills, plugins, and installed extensions stay intact.</p>
        {error ? <div className="form-error" role="alert">{error}</div> : null}
        <div className="dialog-actions">
          <button className="button secondary" type="button" disabled={busy} onClick={onCancel}>Cancel</button>
          <button className="button primary" type="button" disabled={busy} onClick={onConfirm}>{busy ? "Clearing" : "Clear cache"}</button>
        </div>
      </section>
    </div>
  )
}
