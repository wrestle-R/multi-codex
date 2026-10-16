import { useEffect, useRef, useState } from "react"
import { subscribeAppUpdate, updateApp } from "../lib/desktop-api"
import type { UpdateProgress } from "../lib/types"

interface UpdateButtonProps {
  disabled: boolean
  onBusyChange: (busy: boolean) => void
}

export function UpdateButton({ disabled, onBusyChange }: UpdateButtonProps) {
  const [progress, setProgress] = useState<UpdateProgress | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [open, setOpen] = useState(false)
  const running = useRef(false)
  const busy = progress !== null && progress.phase !== "current" && !error
  const percent = progress?.total ? Math.min(100, Math.round(progress.downloaded / progress.total * 100)) : null

  useEffect(() => {
    let active = true
    let unsubscribe: (() => void) | undefined
    void subscribeAppUpdate(event => { if (active) setProgress(event) }).then(dispose => {
      if (active) unsubscribe = dispose
      else dispose()
    }).catch(() => undefined)
    return () => { active = false; unsubscribe?.() }
  }, [])

  useEffect(() => { onBusyChange(busy) }, [busy, onBusyChange])

  async function update() {
    if (disabled || running.current) return
    running.current = true
    setOpen(true)
    setError(null)
    setProgress({ phase: "checking", downloaded: 0, total: null, version: null })
    try {
      const status = await updateApp()
      setProgress(previous => ({ phase: status, downloaded: 0, total: null, version: previous?.version ?? null }))
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error))
    } finally { running.current = false }
  }

  const label = error ? "Update failed" : progress?.phase === "checking" ? "Checking…" : progress?.phase === "downloading" ? `Downloading${percent === null ? "…" : ` ${percent}%`}` : progress?.phase === "installing" ? "Installing…" : progress?.phase === "restarting" ? "Restarting…" : "Update"
  const message = error ? "Couldn’t update" : progress?.phase === "current" ? "You’re up to date" : progress?.phase === "checking" ? "Checking for a new release" : progress?.phase === "downloading" ? `Downloading${progress.version ? ` v${progress.version}` : " the update"}` : progress?.phase === "installing" ? "Verifying and installing" : "Restarting Multi Codex"

  return <div className="update-control">
    <button className={`button secondary update-button${busy ? " is-updating" : ""}`} type="button" disabled={disabled || busy} aria-label={busy ? label : "Update Multi Codex"} aria-busy={busy} onClick={() => void update()}>
      <svg className="update-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" aria-hidden="true"><path d="M20 7v5h-5M4 17v-5h5" /><path d="M6.1 7a7 7 0 0 1 11.5-1.5L20 8M4 16l2.4 2.5A7 7 0 0 0 17.9 17" /></svg>
      <span>{label}</span>
    </button>
    {open ? <div className="update-popover" role={error ? "alert" : "status"} aria-live="polite">
      <div className="update-popover-heading"><strong>{message}</strong>{!busy ? <button className="update-dismiss" type="button" aria-label="Dismiss update status" onClick={() => setOpen(false)}>×</button> : null}</div>
      <p>{error ?? (progress?.phase === "current" ? "You have the latest available version." : "Your saved accounts and chats stay in place.")}</p>
      {busy ? <div className={`update-progress-track${percent === null ? " is-indeterminate" : ""}`} role="progressbar" aria-label="Update progress" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent ?? undefined}><span style={percent === null ? undefined : { width: `${percent}%` }} /></div> : null}
      {error ? <button className="button secondary" type="button" disabled={disabled} onClick={() => void update()}>Try again</button> : null}
    </div> : null}
  </div>
}
