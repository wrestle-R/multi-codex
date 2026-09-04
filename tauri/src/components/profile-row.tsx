import {
  Delete02Icon,
  Edit02Icon,
  PlayIcon,
} from "@hugeicons/core-free-icons"
import { HugeiconsIcon } from "@hugeicons/react"
import type { Profile } from "../lib/types"

interface ProfileRowProps {
  profile: Profile
  onLaunch: (profile: Profile) => void
  onEdit: (profile: Profile) => void
  onDelete: (profile: Profile) => void
}

export function ProfileRow({ profile, onLaunch, onEdit, onDelete }: ProfileRowProps) {
  const busy = profile.status === "launching" || profile.status === "running"

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
        {profile.requestsRemaining != null || profile.resetDate || profile.notes ? (
          <div className="profile-details">
            {profile.requestsRemaining != null ? <span><strong>{profile.requestsRemaining}</strong>{" requests left"}</span> : null}
            {profile.resetDate ? <span>Resets {new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeZone: "UTC" }).format(new Date(`${profile.resetDate}T00:00:00Z`))}</span> : null}
            {profile.notes ? <span className="profile-note" title={profile.notes}>{profile.notes}</span> : null}
          </div>
        ) : null}
        {profile.error ? <p className="inline-error">{profile.error}</p> : null}
      </div>
      <div className="profile-time">
        <span>Updated</span>
        <strong>{new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(profile.updatedAt))}</strong>
      </div>
      <div className="profile-actions">
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
