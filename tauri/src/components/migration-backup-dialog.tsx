import { useEffect, useId, useState } from "react"
import { prepareProfileMigration, previewProfileMigration } from "../lib/desktop-api"
import { formatStorage } from "../lib/formatters"
import type { MigrationPreview, MigrationSnapshot, ProfileStorageUsage } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"

interface Props {
  profile: ProfileStorageUsage
  onClose: () => void
  onPrepared: (snapshot: MigrationSnapshot) => void
}

export function MigrationBackupDialog({ profile, onClose, onPrepared }: Props) {
  const titleId = useId()
  const descriptionId = useId()
  const [preview, setPreview] = useState<MigrationPreview | null>(null)
  const [checking, setChecking] = useState(true)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [revision, setRevision] = useState(0)
  const dialog = useDialogFocus(onClose, busy)

  useEffect(() => {
    let active = true
    setChecking(true)
    setPreview(null)
    setError(null)
    void previewProfileMigration(profile.id).then(value => {
      if (active) setPreview(value)
    }).catch(error => {
      if (active) setError(error instanceof Error ? error.message : String(error))
    }).finally(() => { if (active) setChecking(false) })
    return () => { active = false }
  }, [profile.id, revision])

  const ready = preview && !preview.running && !preview.externalLinks.length
    && preview.availableBackupBytes >= preview.requiredBackupBytes

  async function prepare() {
    if (!ready || checking || busy) return
    setBusy(true)
    setError(null)
    try {
      const snapshot = await prepareProfileMigration(profile.id)
      if (snapshot.state !== "verified") throw new Error("Backup verification is incomplete. Your original profile is kept.")
      onPrepared(snapshot)
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error))
      setPreview(null)
    } finally { setBusy(false) }
  }

  return (
    <div className="dialog-layer elevated-dialog-layer" role="presentation" onMouseDown={event => event.target === event.currentTarget && !busy && onClose()}>
      <section ref={dialog} className="dialog confirm-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId} aria-describedby={descriptionId}>
        <span className="eyebrow">Profile preservation</span>
        <h2 id={titleId}>Back up {profile.name}</h2>
        <p id={descriptionId}>Keep a verified copy of this profile before changing its storage. The copy includes its local chats, settings, workspaces, and profile files. Close its VS Code, Codex Desktop, and CLI sessions first.</p>
        <div className="chat-cleanup-preview" role="status" aria-live="polite">
          {checking ? <p>Checking profile data and backup space…</p> : preview ? <>
            <strong>{formatStorage(preview.sourceBytes)} of profile files</strong>
            <span>Allow {formatStorage(preview.requiredBackupBytes)} for the backup · {formatStorage(preview.availableBackupBytes)} available</span>
            {preview.running ? <p>This profile is in use. Close its sessions, then check again.</p> : null}
            {preview.externalLinks.length ? <p>{preview.externalLinks.length} linked {preview.externalLinks.length === 1 ? "entry needs" : "entries need"} a recovery policy before this profile can be backed up for migration. Your current layout is kept.</p> : null}
            {preview.availableBackupBytes < preview.requiredBackupBytes ? <p>There is not enough free space. Your current data is kept.</p> : null}
            {preview.snapshot?.state === "verified" ? <p>A previous verified migration backup is retained. Changed profile data will receive a new copy.</p> : null}
          </> : null}
          {busy ? <p>Copying and verifying profile files. This may take several minutes.</p> : null}
        </div>
        {error ? <div className="form-error" role="alert">{error}</div> : null}
        <div className="dialog-actions">
          <button className="button secondary" type="button" disabled={busy} onClick={onClose}>Close</button>
          <button className="button secondary" type="button" disabled={checking || busy} onClick={() => setRevision(value => value + 1)}>Check again</button>
          <button className="button primary" type="button" disabled={!ready || checking || busy} onClick={() => void prepare()}>{busy ? "Verifying backup…" : "Create verified backup"}</button>
        </div>
      </section>
    </div>
  )
}
