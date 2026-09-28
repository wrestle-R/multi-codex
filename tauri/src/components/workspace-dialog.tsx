import { useId } from "react"
import type { Profile } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"

interface WorkspaceDialogProps {
  profile: Profile
  workspace: string
  busy: boolean
  onCancel: () => void
  onChoose: (desktop: number | null) => void
}

export function WorkspaceDialog({ profile, workspace, busy, onCancel, onChoose }: WorkspaceDialogProps) {
  const titleId = useId()
  const dialogRef = useDialogFocus(onCancel, busy)
  const segments = workspace.split("/").filter(Boolean)
  const folder = segments[segments.length - 1] ?? workspace

  return (
    <div className="dialog-layer" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && !busy && onCancel()}>
      <section ref={dialogRef} className="dialog workspace-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <span className="eyebrow">Launch destination</span>
        <h2 id={titleId}>Choose a desktop</h2>
        <p>Open <strong>{folder}</strong> with {profile.name} on your current desktop or move directly to another one.</p>
        <button className="desktop-current" type="button" disabled={busy} onClick={() => onChoose(null)}>
          <span>Current desktop</span>
          <small>Open here</small>
        </button>
        <div className="desktop-grid" aria-label="Hyprland desktops">
          {Array.from({ length: 10 }, (_, index) => index + 1).map((desktop) => (
            <button key={desktop} type="button" disabled={busy} aria-label={`Open on desktop ${desktop}`} onClick={() => onChoose(desktop)}>
              {desktop}
            </button>
          ))}
        </div>
        <div className="dialog-actions">
          <button className="button secondary" type="button" disabled={busy} onClick={onCancel}>Cancel</button>
        </div>
      </section>
    </div>
  )
}
