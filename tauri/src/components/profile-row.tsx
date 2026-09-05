import {
  Delete02Icon,
  Edit02Icon,
  PlayIcon,
  Refresh01Icon,
} from "@hugeicons/core-free-icons"
import { HugeiconsIcon } from "@hugeicons/react"
import type { LimitCheckState, LimitWindow, Profile } from "../lib/types"

interface ProfileRowProps {
  profile: Profile
  limits?: LimitCheckState
  onCheckLimits: (profile: Profile) => void
  onLaunch: (profile: Profile) => void
  onEdit: (profile: Profile) => void
  onDelete: (profile: Profile) => void
}

function resetLabel(window: LimitWindow | null): string {
  if (!window?.resetsAt) return "Reset unavailable"
  const reset = new Date(window.resetsAt * 1000)
  const deltaMinutes = Math.max(0, Math.ceil((reset.getTime() - Date.now()) / 60_000))
  const countdown = deltaMinutes >= 1440
    ? `${Math.floor(deltaMinutes / 1440)}d ${Math.floor((deltaMinutes % 1440) / 60)}h`
    : deltaMinutes >= 60
      ? `${Math.floor(deltaMinutes / 60)}h ${deltaMinutes % 60}m`
      : `${deltaMinutes}m`
  return `Resets in ${countdown}`
}

function LimitMetric({ label, window }: { label: string; window: LimitWindow | null }) {
  const value = window?.remainingPercent
  return (
    <div className="limit-metric">
      <div className="limit-metric-heading">
        <span>{label}</span>
        <strong>{value == null ? "Unavailable" : `${value}% left`}</strong>
      </div>
      <div
        className="limit-track"
        role={value == null ? undefined : "progressbar"}
        aria-label={value == null ? undefined : `${label} usage remaining`}
        aria-valuemin={value == null ? undefined : 0}
        aria-valuemax={value == null ? undefined : 100}
        aria-valuenow={value}
      >
        {value != null ? <span style={{ width: `${value}%` }} /> : null}
      </div>
      <small title={window?.resetsAt ? new Date(window.resetsAt * 1000).toLocaleString() : undefined}>
        {resetLabel(window)}
      </small>
    </div>
  )
}

export function ProfileRow({ profile, limits, onCheckLimits, onLaunch, onEdit, onDelete }: ProfileRowProps) {
  const busy = profile.status === "launching" || profile.status === "running"
  const supportsLimits = profile.authMode.toLowerCase() === "chatgpt"

  return (
    <article className="profile-row" data-testid={`profile-${profile.id}`}>
      <div className="profile-avatar" aria-hidden="true">
        {profile.name.slice(0, 1).toUpperCase()}
      </div>
      <div className="profile-main">
        <div className="profile-heading">
          <h2>{profile.name}</h2>
          <span className={`status status-${profile.status}`}>{profile.status}</span>
        </div>
        <p>{profile.authMode} account</p>
        {profile.notes ? (
          <div className="profile-details">
            {profile.notes ? <span className="profile-note" title={profile.notes}>{profile.notes}</span> : null}
          </div>
        ) : null}
        {limits?.data ? (
          <div className="limits-panel" aria-label={`Live limits for ${profile.name}`}>
            <LimitMetric label="5-hour" window={limits.data.fiveHour} />
            <LimitMetric label="Weekly" window={limits.data.weekly} />
            <div className="reset-credit-metric">
              <span>Reset credits</span>
              <strong>{limits.data.resetCreditsAvailable ?? "Unavailable"}</strong>
              <small>Checked {new Intl.DateTimeFormat(undefined, { timeStyle: "short" }).format(new Date(limits.data.checkedAt))}</small>
            </div>
          </div>
        ) : null}
        {limits?.error ? <p className="inline-error limits-error">{limits.error}</p> : null}
        {profile.error ? <p className="inline-error">{profile.error}</p> : null}
      </div>
      <div className="profile-time">
        <span>Updated</span>
        <strong>{new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(profile.updatedAt))}</strong>
      </div>
      <div className="profile-actions">
        <button
          className="button secondary limits-button"
          type="button"
          title={supportsLimits ? `Check live limits for ${profile.name}` : "Live limits require a ChatGPT account"}
          disabled={!supportsLimits || limits?.loading}
          onClick={() => onCheckLimits(profile)}
        >
          <HugeiconsIcon icon={Refresh01Icon} size={18} strokeWidth={1.8} />
          {supportsLimits ? limits?.loading ? "Checking" : limits?.data ? "Refresh" : "Check limits" : "Unavailable"}
        </button>
        <button
          className="icon-button"
          type="button"
          title={`Edit ${profile.name}`}
          aria-label={`Edit ${profile.name}`}
          onClick={() => onEdit(profile)}
        >
          <HugeiconsIcon icon={Edit02Icon} size={19} strokeWidth={1.8} />
        </button>
        <button
          className="icon-button danger-button"
          type="button"
          title={`Delete ${profile.name}`}
          aria-label={`Delete ${profile.name}`}
          disabled={busy}
          onClick={() => onDelete(profile)}
        >
          <HugeiconsIcon icon={Delete02Icon} size={19} strokeWidth={1.8} />
        </button>
        <button
          className="button primary launch-button"
          type="button"
          disabled={busy}
          onClick={() => onLaunch(profile)}
        >
          <HugeiconsIcon icon={PlayIcon} size={18} strokeWidth={1.8} />
          {profile.status === "launching" ? "Opening" : profile.status === "running" ? "Running" : "Launch"}
        </button>
      </div>
    </article>
  )
}
