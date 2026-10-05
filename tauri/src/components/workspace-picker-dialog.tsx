import { useEffect, useId, useRef, useState } from "react"
import { listWorkspaceDirectories } from "../lib/desktop-api"
import type { WorkspaceDirectoryListing } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"

interface WorkspacePickerDialogProps {
  purpose?: "launch" | "preference"
  initialPath: string
  busy: boolean
  onCancel: () => void
  onChoose: (path: string) => void
}

export function WorkspacePickerDialog({ initialPath, busy, onCancel, onChoose, purpose = "launch" }: WorkspacePickerDialogProps) {
  const titleId = useId()
  const dialogRef = useDialogFocus(onCancel, busy)
  const navigationSequence = useRef(0)
  const [listing, setListing] = useState<WorkspaceDirectoryListing | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function navigate(path: string) {
    const sequence = ++navigationSequence.current
    setLoading(true)
    setError(null)
    try {
      const next = await listWorkspaceDirectories(path)
      if (sequence === navigationSequence.current) setListing(next)
    } catch (cause) {
      if (sequence === navigationSequence.current) setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      if (sequence === navigationSequence.current) setLoading(false)
    }
  }

  useEffect(() => {
    void navigate(initialPath)
    return () => { navigationSequence.current++ }
  }, [initialPath])

  const segments = listing?.path.split("/").filter(Boolean) ?? []
  const parentSegments = segments.slice(0, -1)
  const parentPath = listing?.parentPath ?? (segments.length > 1 ? `/${parentSegments.join("/")}` : null)

  return (
    <div className="dialog-layer" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && !busy && onCancel()}>
      <section ref={dialogRef} className="dialog folder-picker-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <div className="dialog-header">
          <div>
            <span className="eyebrow">{purpose === "preference" ? "Preferred folder" : "Launch workspace"}</span>
            <h2 id={titleId}>Choose a folder</h2>
          </div>
          <button className="dialog-close" type="button" aria-label="Close folder picker" disabled={busy} onClick={onCancel}>×</button>
        </div>
        <p className="folder-picker-hint">{purpose === "preference" ? "Start future folder selections here. You can choose another project each time." : "Select a project folder for this profile. Nothing in the folder will be changed."}</p>
        <div className="folder-picker-location">
          <button type="button" disabled={!parentPath || loading || busy} onClick={() => parentPath && void navigate(parentPath)}>↑ Up</button>
          <span title={listing?.path}>{listing?.path ?? initialPath}</span>
        </div>
        <div className="folder-picker-list" aria-label="Folders" aria-busy={loading}>
          {loading ? <p className="folder-picker-empty">Reading folders…</p> : null}
          {!loading && error ? <p className="form-error">{error}</p> : null}
          {!loading && !error && listing?.directories.length === 0 ? <p className="folder-picker-empty">No subfolders here. You can choose this folder.</p> : null}
          {!loading && !error && listing?.directories.map((directory) => (
            <button className="folder-picker-entry" key={directory.path} type="button" disabled={busy} onClick={() => void navigate(directory.path)}>
              <span className="folder-glyph" aria-hidden="true">▰</span>
              <span title={directory.name}>{directory.name}</span>
              <span className="folder-entry-open">Open →</span>
            </button>
          ))}
        </div>
        {error ? <button className="folder-picker-retry" type="button" onClick={() => void navigate(listing?.path ?? initialPath)}>Try again</button> : null}
        <div className="dialog-actions folder-picker-actions">
          <button className="button secondary" type="button" disabled={busy} onClick={onCancel}>Cancel</button>
          <button className="button primary" type="button" disabled={!listing || loading || Boolean(error) || busy} onClick={() => listing && onChoose(listing.path)}>Choose this folder</button>
        </div>
      </section>
    </div>
  )
}
