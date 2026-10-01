import { useEffect, useId, useState } from "react"
import { getDesktopInventory } from "../lib/desktop-api"
import type { DesktopInventory, DesktopWindow, Profile } from "../lib/types"
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
  const [previewed, setPreviewed] = useState<string | null>(null)
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
  const preview = inventory?.desktops.find(desktop => desktop.id === previewed)
    ?? inventory?.desktops.find(desktop => desktop.current && desktop.windows.length > 0)
    ?? inventory?.desktops.find(desktop => desktop.windows.length > 0)
    ?? inventory?.desktops[0]

  return (
    <div className="dialog-layer" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && !busy && onCancel()}>
      <section ref={dialogRef} className="dialog workspace-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId} aria-busy={busy}>
        <span className="eyebrow">Launch destination</span>
        <h2 id={titleId}>Choose a desktop</h2>
        <p>Open <strong>{folder}</strong> with {profile.name}. App icons show what is open on each desktop.</p>
        <button className="desktop-current" type="button" disabled={busy} onClick={() => { setSelected(null); onChoose(null) }}>
          <span>{alreadyOpened ? "Keep the opened window" : "Current desktop"}</span>
          <small>{alreadyOpened ? "Finish without moving" : "Open here"}</small>
        </button>
        {loading ? <p className="desktop-notice" role="status">Reading desktops and windows…</p> : null}
        {inventory?.capabilities.reason ? <p className="desktop-notice" role="status">{inventory.capabilities.reason}</p> : null}
        {inventoryError ? <p className="form-error" role="alert">{inventoryError}</p> : null}
        {disappeared ? <p className="desktop-notice" role="status">The selected desktop disappeared. Choose another destination.</p> : null}
        <div className="desktop-grid" aria-label="Available desktops">
          {inventory?.desktops.map(desktop => {
            const applications = [...new Map(desktop.windows.map(window => [window.application, window])).values()]
            return (
              <button key={desktop.id} type="button" className={`desktop-preview${desktop.current ? " desktop-active" : ""}${selected === desktop.id ? " desktop-selected" : ""}`} disabled={busy || !canMove} aria-label={`Open on ${desktop.name}`} aria-pressed={selected === desktop.id}
                title={desktop.windows.map(window => `${window.application}: ${window.title || "Untitled window"}`).join("\n") || "No open windows"}
                onMouseEnter={() => setPreviewed(desktop.id)} onFocus={() => setPreviewed(desktop.id)}
                onClick={() => { setSelected(desktop.id); onChoose(desktop.id) }}>
                <span className="desktop-preview-heading"><strong>{desktop.name.replace(/^Desktop /, "")}</strong>{desktop.current ? <span className="desktop-current-marker">Current</span> : null}</span>
                <span className="desktop-app-icons">
                  {applications.length === 0 ? <svg className="desktop-empty-icon" viewBox="0 0 28 24" aria-hidden="true"><rect x="2" y="2" width="24" height="16" rx="2" /><path d="M9 22h10M14 18v4" /></svg> : applications.slice(0, 4).map(window => <ApplicationIcon key={window.application} window={window} />)}
                  {applications.length > 4 ? <span className="desktop-app-overflow">+{applications.length - 4}</span> : null}
                </span>
                <span className="desktop-preview-meta">{desktop.windows.length === 0 ? "Empty desktop" : `${desktop.windows.length} ${desktop.windows.length === 1 ? "window" : "windows"}`}</span>
                <span className="desktop-app-names">{applications.map(window => window.application).join(" · ") || "Ready for a new window"}</span>
                {desktop.windows.length === 0 ? <span className="sr-only">No open windows</span> : null}
              </button>
            )
          })}
        </div>
        {preview ? <div className="desktop-inspector" aria-label={`Windows on ${preview.name}`}>
          <span className="desktop-inspector-heading">{preview.name}{preview.monitor ? <small>{preview.monitor}</small> : null}</span>
          <div className="desktop-window-list">
            {preview.windows.length === 0 ? <span className="desktop-empty">Ready for a new window</span> : preview.windows.map(window => (
              <span className="desktop-window" key={window.id}><ApplicationIcon window={window} /><span className="desktop-window-copy"><span className="desktop-window-app">{window.application}</span><span className="desktop-window-title" title={window.title}>{window.title || "Untitled window"}</span></span></span>
            ))}
          </div>
        </div> : null}
        {!loading && canMove && inventory?.desktops.length === 0 ? <p className="desktop-notice">No desktops are currently available.</p> : null}
        {error ? <p className="form-error" role="alert">{error}{alreadyOpened ? " Your window is already open. Select a desktop to retry placement." : ""}</p> : null}
        {busy ? <p className="desktop-notice" role="status">{alreadyOpened ? "Retrying placement…" : "Opening VS Code and verifying destination…"}</p> : null}
        <div className="dialog-actions desktop-picker-actions">
          <span className="desktop-finish-note">Closes automatically when your window is ready.</span>
          <button className="button secondary" type="button" disabled={busy} onClick={() => setRefreshKey(key => key + 1)}>Refresh desktops</button>
          <button className="button secondary" type="button" disabled={busy} onClick={onCancel}>{alreadyOpened ? "Close" : "Cancel"}</button>
        </div>
      </section>
    </div>
  )
}

function ApplicationIcon({ window }: { window: DesktopWindow }) {
  const [failedIcon, setFailedIcon] = useState<string | null>(null)
  return window.icon && failedIcon !== window.icon
    ? <img className="desktop-app-icon" src={window.icon} alt={window.application} title={window.application} onError={() => setFailedIcon(window.icon ?? null)} />
    : <span className="desktop-app-fallback" role="img" aria-label={window.application} title={window.application}>{window.application.split(".").pop()?.slice(0, 1).toUpperCase() || "?"}</span>
}
