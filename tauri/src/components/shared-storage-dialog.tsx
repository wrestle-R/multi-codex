import { useEffect, useId, useState } from "react"
import { migrateProfileSharedStorage, previewProfileMigration, rollbackProfileSharedStorage, restoreProfileBackup, previewLegacyExtensions, clearLegacyExtensions } from "../lib/desktop-api"
import { formatStorage } from "../lib/formatters"
import type { MigrationPreview, ProfileStorageUsage } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"

export function SharedStorageDialog({ profile, onClose, onCompleted }: {
  profile: ProfileStorageUsage; onClose: () => void; onCompleted: (message: string) => void
}) {
  const titleId = useId(); const descriptionId = useId()
  const [preview, setPreview] = useState<MigrationPreview | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [checking, setChecking] = useState(true)
  const [busy, setBusy] = useState(false)
  const [revision, setRevision] = useState(0)
  const [legacyPreview, setLegacyPreview] = useState<{ token: string; bytes: number } | null>(null)
  const [restoreRequested, setRestoreRequested] = useState(false)
  const restoring = profile.layout === "needsRestore"
  const recovery = profile.layout === "shared" || profile.layout === "needsRecovery" || restoring
  const dialog = useDialogFocus(onClose, busy)
  useEffect(() => {
    let active = true; setPreview(null); setError(null); setChecking(true)
    void previewProfileMigration(profile.id).then(value => { if (active) setPreview(value) })
      .catch(error => { if (active) setError(error instanceof Error ? error.message : String(error)) })
      .finally(() => { if (active) setChecking(false) })
    return () => { active = false }
  }, [profile.id, revision])
  const ready = preview && !preview.running && (recovery || (!preview.externalLinks.length
    && preview.availableBackupBytes >= preview.requiredBackupBytes
    && preview.requiredAppBytes !== undefined && preview.availableAppBytes !== undefined
    && preview.availableAppBytes >= preview.requiredAppBytes))
  async function clearOriginals() {
    if (busy || checking || !preview || preview.running) return
    setBusy(true); setError(null)
    try {
      if (!legacyPreview) { setLegacyPreview(await previewLegacyExtensions(profile.id)); return }
      const bytes = await clearLegacyExtensions(profile.id, legacyPreview.token)
      onCompleted(`Cleared ${formatStorage(bytes)} of original extension copies for ${profile.name}. Current shared packages, chats, login, and verified recovery backup are retained.`)
    } catch (error) { setError(error instanceof Error ? error.message : String(error)); setLegacyPreview(null) }
    finally { setBusy(false) }
  }
  async function restore() {
    if (busy || !preview || preview.running || preview.snapshot?.state !== "verified") return
    setBusy(true); setError(null)
    try {
      const result = await restoreProfileBackup(profile.id)
      if (result.phase !== "complete") throw new Error("Restore needs recovery. Current profile data is retained.")
      onCompleted(`Backup restored for ${profile.name}. Its previous profile files, current login, shared chats, and backups are retained.`)
    } catch (error) { setError(error instanceof Error ? error.message : String(error)); setPreview(null) }
    finally { setBusy(false) }
  }
  async function repair() {
    if (busy || !ready || checking) return
    setBusy(true); setError(null)
    try {
      const result = await migrateProfileSharedStorage(profile.id)
      if (result.phase !== "committed") throw new Error("Shared storage repair is incomplete.")
      onCompleted(`Shared storage repaired for ${profile.name}. Original data and verified backups are retained.`)
    } catch (error) { setError(error instanceof Error ? error.message : String(error)); setPreview(null) }
    finally { setBusy(false) }
  }
  async function apply() {
    if (restoring) { await restore(); return }
    if (!ready || busy || checking) return
    setBusy(true); setError(null)
    try {
      const result = recovery ? await rollbackProfileSharedStorage(profile.id) : await migrateProfileSharedStorage(profile.id)
      if (result.phase !== (recovery ? "rolledBack" : "committed")) throw new Error("Storage recovery is incomplete. Original data has been retained.")
      onCompleted(recovery ? `Original VS Code layout selected for ${profile.name}. Shared chats and backups are retained.`
        : `Shared Codex storage is ready for ${profile.name}. Its verified backup and original extension folder are retained.`)
    } catch (error) { setError(error instanceof Error ? error.message : String(error)); setPreview(null) }
    finally { setBusy(false) }
  }
  return <div className="dialog-layer elevated-dialog-layer" role="presentation" onMouseDown={event => event.target === event.currentTarget && !busy && onClose()}>
    <section ref={dialog} className="dialog confirm-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId} aria-describedby={descriptionId}>
      <span className="eyebrow">Shared Codex storage</span>
      <h2 id={titleId}>{restoring ? "Finish profile restore" : recovery ? "Recover the original layout" : `Share Codex for ${profile.name}`}</h2>
      <p id={descriptionId}>{recovery
        ? "Select the retained original extension folder. Later user settings edits are kept, and shared conversations remain accessible from other shared profiles. Close this profile's sessions first."
        : "Use one managed Codex package set and let saved accounts open and continue these chats. Each account keeps its login and settings. Copilot is preserved. A verified backup and the original extension folder are kept."}</p>
      {legacyPreview ? <p role="status">Remove {formatStorage(legacyPreview.bytes)} from this profile's original extension folder. Its verified backup remains available for rollback. Shared packages, settings, chats, and credentials are kept.</p> : null}
      {restoreRequested ? <p role="status">Restore the last verified backup into this profile. The current profile is retained separately, your current login is preserved, and shared chats remain available. Close all profile sessions first.</p> : null}
      {!recovery ? <p>Other existing extensions stay in the retained original folder. This setup starts with Codex and preserves Copilot and its required packages.</p> : null}
      <div className="chat-cleanup-preview" role="status" aria-live="polite">
        {checking ? <p>Checking profile sessions and migration space…</p> : preview ? <>
          {preview.running ? <p>Close this profile's VS Code, Codex Desktop, and CLI sessions, then check again.</p> : null}
          {!recovery ? <>
            <strong>{formatStorage(preview.sourceBytes)} of current profile data</strong>
            <span>Backup reserve: {formatStorage(preview.requiredBackupBytes)} · {formatStorage(preview.availableBackupBytes)} available</span>
            <span>Conservative migration reserve: {formatStorage(preview.requiredAppBytes ?? 0)} · {formatStorage(preview.availableAppBytes ?? 0)} available</span>
            {preview.externalLinks.length ? <p>Linked entries need complete backup coverage before migration. The current layout is kept.</p> : null}
            {!ready && !preview.running && !preview.externalLinks.length ? <p>More free space or a complete migration preview is required.</p> : null}
          </> : null}
        </> : null}
        {busy ? <p>{recovery ? "Recovering the original layout…" : "Preparing shared packages, verifying a backup, and importing complete chats. This may take several minutes."}</p> : null}
      </div>
      {error ? <div className="form-error" role="alert">{error}</div> : null}
      <div className="dialog-actions">
        <button className="button secondary" disabled={busy} onClick={onClose}>Close</button>
        <button className="button secondary" disabled={busy || checking} onClick={() => setRevision(value => value + 1)}>Check again</button>
        {profile.layout === "shared" && !restoreRequested ? <button className="button secondary" disabled={busy || checking || !preview || preview.running} onClick={() => void clearOriginals()}>{legacyPreview ? "Confirm original extension cleanup" : "Preview original extension cleanup"}</button> : null}
        {profile.layout === "shared" && !restoreRequested ? <button className="button secondary" disabled={!ready || busy || checking} onClick={() => void repair()}>Repair shared storage</button> : null}
        {preview?.snapshot?.state === "verified" ? <button className="button secondary" disabled={busy || checking || preview.running} onClick={() => restoreRequested ? void restore() : setRestoreRequested(true)}>{restoreRequested ? "Confirm backup restore" : "Restore backup"}</button> : null}
        <button className="button primary" disabled={!ready || busy || checking || restoreRequested || !!legacyPreview} onClick={() => void apply()}>{busy ? "Working…" : restoring ? "Finish interrupted restore" : recovery ? "Return to original layout" : "Back up and enable sharing"}</button>
      </div>
    </section>
  </div>
}
