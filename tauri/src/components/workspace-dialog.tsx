import { useEffect, useId, useState } from "react"
import { getDesktopInventory } from "../lib/desktop-api"
import type { DesktopInventory, Profile } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"

interface WorkspaceDialogProps {
  profile: Profile
  workspace: string
  busy: boolean
  error: string | null
  alreadyOpened: boolean
  onCancel: () => void
  onChoose: (desktop: string | null) => void
}

export function WorkspaceDialog({ profile, workspace, busy, error, alreadyOpened, onCancel, onChoose }: WorkspaceDialogProps) {
  const titleId = useId()
  const dialogRef = useDialogFocus(onCancel, busy)
  const [inventory, setInventory] = useState<DesktopInventory | null>(null)
  const [inventoryError, setInventoryError] = useState<string | null>(null)
  const [selected, setSelected] = useState<string | null>(null)
  const [refreshKey, setRefreshKey] = useState(0)
  const [loading, setLoading] = useState(true)
  const segments = workspace.split("/").filter(Boolean)
  const folder = segments[segments.length - 1] ?? workspace

  useEffect(() => {
    let cancelled = false
    let timer: ReturnType<typeof setTimeout> | undefined
    async function poll() {
      if (busy) return
      try {
        const next = await getDesktopInventory()
        if (cancelled) return
        if (next.protocolVersion !== 1) throw new Error("Desktop integration requires an update")
        setInventory(next)
        setInventoryError(null)
      } catch (error) {
        if (!cancelled) setInventoryError(error instanceof Error ? error.message : String(error))
      } finally {
        if (!cancelled) {
          setLoading(false)
          timer = setTimeout(() => void poll(), 2000)
        }
      }
    }
    void poll()
    return () => { cancelled = true; clearTimeout(timer) }
  }, [busy, refreshKey])

  const disappeared = selected !== null && inventory !== null && !inventory.desktops.some(desktop => desktop.id === selected)
  const canMove = inventory?.capabilities.moveWindows && !inventoryError

  return (
    <div className="dialog-layer" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && !busy && onCancel()}>
      <section ref={dialogRef} className="dialog workspace-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId} aria-busy={busy}>
        <span className="eyebrow">Launch destination</span>
        <h2 id={titleId}>Choose a desktop</h2>
        <p>Open <strong>{folder}</strong> with {profile.name}. Choose a desktop by its open windows.</p>
        <button className="desktop-current" type="button" disabled={busy} onClick={() => { setSelected(null); onChoose(null) }}>
          <span>{alreadyOpened ? "Keep the opened window" : "Current desktop"}</span>
          <small>{alreadyOpened ? "Finish without moving" : "Open here"}</small>
        </button>
        {loading ? <p className="desktop-notice" role="status">Reading desktops and windows…</p> : null}
        {inventory?.capabilities.reason ? <p className="desktop-notice" role="status">{inventory.capabilities.reason}</p> : null}
        {inventoryError ? <p className="form-error" role="alert">{inventoryError}</p> : null}
        {disappeared ? <p className="desktop-notice" role="status">The selected desktop disappeared. Choose another destination.</p> : null}
        <div className="desktop-grid" aria-label="Available desktops">
          {inventory?.desktops.map(desktop => (
            <button key={desktop.id} type="button" className={`desktop-preview${selected === desktop.id ? " desktop-selected" : ""}`} disabled={busy || !canMove} aria-label={`Open on ${desktop.name}`} aria-pressed={selected === desktop.id} onClick={() => { setSelected(desktop.id); onChoose(desktop.id) }}>
              <span className="desktop-preview-heading"><strong>{desktop.name}</strong>{desktop.current ? <span className="desktop-current-marker">Current</span> : null}</span>
              <span className="desktop-preview-meta">{desktop.windows.length} {desktop.windows.length === 1 ? "window" : "windows"}{desktop.monitor ? ` · ${desktop.monitor}` : ""}</span>
              <span className="desktop-window-list">
                {desktop.windows.length === 0 ? <span className="desktop-empty">No open windows</span> : desktop.windows.map(window => (
                  <span className="desktop-window" key={window.id}><span className="desktop-window-app">{window.application}</span><span className="desktop-window-title" title={window.title}>{window.title || "Untitled window"}</span></span>
                ))}
              </span>
            </button>
          ))}
        </div>
        {!loading && canMove && inventory?.desktops.length === 0 ? <p className="desktop-notice">No desktops are currently available.</p> : null}
        {error ? <p className="form-error" role="alert">{error}{alreadyOpened ? " Your window is already open. Select a desktop to retry placement." : ""}</p> : null}
        {busy ? <p className="desktop-notice" role="status">{alreadyOpened ? "Retrying placement…" : "Opening VS Code and verifying destination…"}</p> : null}
        <div className="dialog-actions">
          <button className="button secondary" type="button" disabled={busy} onClick={() => setRefreshKey(key => key + 1)}>Refresh desktops</button>
          <button className="button secondary" type="button" disabled={busy} onClick={onCancel}>{alreadyOpened ? "Close" : "Cancel"}</button>
        </div>
      </section>
    </div>
  )
}
