import { useId } from "react"
import type { ProfileLimits } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"

interface ResetCreditsDialogProps {
  profileName: string
  limits: ProfileLimits
  onClose: () => void
}

function expiryLabel(timestamp: number | null): string {
  if (timestamp == null) return "Does not expire"
  const date = new Date(timestamp * 1000)
  const minutes = Math.ceil((date.getTime() - Date.now()) / 60_000)
  if (minutes <= 0) return "Expired"
  if (minutes < 60) return `Expires in ${minutes}m`
  if (minutes < 1440) return `Expires in ${Math.floor(minutes / 60)}h ${minutes % 60}m`
  return `Expires in ${Math.floor(minutes / 1440)}d ${Math.floor((minutes % 1440) / 60)}h`
}

export function ResetCreditsDialog({ profileName, limits, onClose }: ResetCreditsDialogProps) {
  const titleId = useId()
  const dialogRef = useDialogFocus(onClose, false)
  return (
    <div className="dialog-layer" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <section ref={dialogRef} className="dialog credits-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <div className="dialog-header">
          <div><span className="eyebrow">{profileName}</span><h2 id={titleId}>Reset credits</h2></div>
          <button className="dialog-close" type="button" aria-label="Close" onClick={onClose}>×</button>
        </div>
        <p>{limits.resetCreditsAvailable ?? 0} available</p>
        {limits.resetCredits == null ? (
          <div className="notice">Codex reported the count, but did not provide expiry details for these credits.</div>
        ) : limits.resetCredits.length === 0 ? (
          <div className="notice">No available reset-credit details were returned.</div>
        ) : (
          <div className="credit-list">
            {limits.resetCredits.map((credit, index) => (
              <article key={credit.id}>
                <div><strong>{credit.title || `Reset credit ${index + 1}`}</strong><span>{expiryLabel(credit.expiresAt)}</span></div>
                {credit.expiresAt != null ? <time dateTime={new Date(credit.expiresAt * 1000).toISOString()}>{new Date(credit.expiresAt * 1000).toLocaleString()}</time> : null}
                {credit.description ? <p>{credit.description}</p> : null}
              </article>
            ))}
          </div>
        )}
      </section>
    </div>
  )
}
