import {
  Delete02Icon,
  Edit02Icon,
  Refresh01Icon,
} from "@hugeicons/core-free-icons"
import { HugeiconsIcon } from "@hugeicons/react"
import type { LaunchTarget, LimitCheckState, LimitWindow, Profile, ProfileLimits } from "../lib/types"

interface ProfileRowProps {
  profile: Profile
  limits?: LimitCheckState
  onCheckLimits: (profile: Profile) => void
  onLaunch: (profile: Profile, target: LaunchTarget) => void
  launchTargets?: LaunchTarget[]
  onEdit: (profile: Profile) => void
  onDelete: (profile: Profile) => void
  onShowResetCredits: (profile: Profile, limits: ProfileLimits) => void
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

export function ProfileRow({ profile, limits, onCheckLimits, onLaunch, launchTargets = ['vscode'], onEdit, onDelete, onShowResetCredits }: ProfileRowProps) {
  const supportsLimits = profile.authMode.toLowerCase() === "chatgpt"
  const hasReportedWindow = Boolean(limits?.data?.fiveHour || limits?.data?.weekly || limits?.data?.monthly)
  const freeLimitsNotReported = profile.accountTier?.toLowerCase() === "free" && limits?.data && !hasReportedWindow

  return (
    <article className="profile-row" data-testid={`profile-${profile.id}`}>
      <div className="profile-avatar" aria-hidden="true">
        {profile.name.slice(0, 1).toUpperCase()}
      </div>
      <div className="profile-main">
        <div className="profile-heading">
          <h2>{profile.name}</h2>
          {profile.accountTier ? <span className="account-tier">{profile.accountTier}</span> : null}
        </div>
        <p>{profile.authMode} account</p>
        {profile.notes ? (
          <div className="profile-details">
            {profile.notes ? <span className="profile-note" title={profile.notes}>{profile.notes}</span> : null}
          </div>
        ) : null}
        {limits?.data ? (
          <div className="limits-panel" aria-label={`Live limits for ${profile.name}`}>
            {limits.data.monthly ? <LimitMetric label="Monthly" window={limits.data.monthly} /> : null}
            {!limits.data.monthly && !freeLimitsNotReported ? <LimitMetric label="5-hour" window={limits.data.fiveHour} /> : null}
            {!limits.data.monthly && !freeLimitsNotReported ? <LimitMetric label="Weekly" window={limits.data.weekly} /> : null}
            {freeLimitsNotReported ? <p className="free-limits-note">Free account limits not reported</p> : null}
            <button className="reset-credit-metric" type="button" onClick={() => onShowResetCredits(profile, limits.data!)} aria-label={`Show reset-credit expiry for ${profile.name}`}>
              <span>Reset credits</span>
              <strong>{limits.data.resetCreditsAvailable ?? "Unavailable"}</strong>
              <small>{limits.data.resetCreditsAvailable == null ? "No details" : "View expiry"}</small>
            </button>
            <span className="limits-freshness" title={new Date(limits.data.checkedAt).toLocaleString()}>
              {limits.loading ? "Updating usage…" : limits.error ? "Showing last saved usage" : `Updated ${new Date(limits.data.checkedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`}
            </span>
          </div>
        ) : null}
        {limits?.error ? <p className="inline-error limits-error">{limits.error}</p> : null}
        {profile.error ? <p className="inline-error">{profile.error}</p> : null}
      </div>
      <div className="profile-actions">
        <div className="profile-management">
          <button
            className={`button secondary limits-button${limits?.loading ? " is-refreshing" : ""}`}
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
            onClick={() => onDelete(profile)}
          >
            <HugeiconsIcon icon={Delete02Icon} size={19} strokeWidth={1.8} />
          </button>
        </div>
        <div className="profile-launchers">
          {launchTargets.map(target => <button
            key={target}
            className={`button ${launchTargets.length > 1 && target !== 'standalone' ? 'secondary' : 'primary'} launch-button`}
            type="button"
            onClick={() => onLaunch(profile, target)}
          >
            {target === 'cli' ? <span className="terminal-glyph" aria-hidden="true">›_</span> : target === 'vscode' ? <img className="launch-logo" src="/logos/vscode.svg" alt="" aria-hidden="true" /> : <span className="launch-logo codex-logo" aria-hidden="true" />}
            {target === 'cli' ? 'CLI' : target === 'vscode' ? 'VS Code' : 'Codex'}
          </button>)}
        </div>
      </div>
    </article>
  )
}
