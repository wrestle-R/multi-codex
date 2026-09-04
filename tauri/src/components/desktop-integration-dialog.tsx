import { ComputerIcon } from "@hugeicons/core-free-icons"
import { HugeiconsIcon } from "@hugeicons/react"
import { useId, useState } from "react"
import { useDialogFocus } from "./use-dialog-focus"

interface DesktopIntegrationDialogProps {
  busy: boolean
  error: string | null
  onDismiss: () => void
  onInstall: (createDesktopShortcut: boolean) => Promise<void>
}

export function DesktopIntegrationDialog({
  busy,
  error,
  onDismiss,
  onInstall,
}: DesktopIntegrationDialogProps) {
  const titleId = useId()
  const [createDesktopShortcut, setCreateDesktopShortcut] = useState(true)
  const dialogRef = useDialogFocus(onDismiss, busy)

  return (
    <div className="dialog-layer" role="presentation">
      <section ref={dialogRef} className="dialog integration-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <div className="integration-icon" aria-hidden="true">
          <HugeiconsIcon icon={ComputerIcon} size={26} strokeWidth={1.7} />
        </div>
        <span className="eyebrow">Desktop integration</span>
        <h2 id={titleId}>Add Multi Codex to your apps?</h2>
        <p>Install this AppImage for your user account so it opens from the app drawer with the correct icon.</p>

        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={createDesktopShortcut}
            disabled={busy}
            onChange={(event) => setCreateDesktopShortcut(event.currentTarget.checked)}
          />
          <span>
            <strong>Create Desktop shortcut</strong>
            <small>Also place a launcher in your Desktop folder.</small>
          </span>
        </label>

        {error ? <div className="form-error" role="alert">{error}</div> : null}

        <div className="dialog-actions">
          <button className="button secondary" type="button" disabled={busy} onClick={onDismiss}>Not now</button>
          <button className="button primary" type="button" disabled={busy} onClick={() => void onInstall(createDesktopShortcut)}>
            {busy ? "Installing" : "Install"}
          </button>
        </div>
      </section>
    </div>
  )
}
