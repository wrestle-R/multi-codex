import { useEffect, useMemo, useState } from "react"
import { claimReminder, expiringResets, type ExpiringResetGroup } from "../lib/reset-reminders"
import type { LimitCheckState, Profile } from "../lib/types"

export function ResetExpiryReminder({ profiles, checks, ready, onView }: {
  profiles: Profile[]
  checks: Record<string, LimitCheckState>
  ready: boolean
  onView: (group: ExpiringResetGroup) => void
}) {
  const [now, setNow] = useState(Date.now)
  const [visible, setVisible] = useState(false)
  const [expanded, setExpanded] = useState(false)
  const [hovered, setHovered] = useState(false)
  const [focused, setFocused] = useState(false)
  const groups = useMemo(() => expiringResets(profiles, checks, now), [profiles, checks, now])
  useEffect(() => {
    const update = () => setNow(Date.now())
    const timer = window.setInterval(update, 60_000)
    document.addEventListener("visibilitychange", update)
    window.addEventListener("focus", update)
    return () => {
      window.clearInterval(timer)
      document.removeEventListener("visibilitychange", update)
      window.removeEventListener("focus", update)
    }
  }, [])
  useEffect(() => {
    if (!ready || !groups.length || visible || document.visibilityState === "hidden") return
    // Wait for the account refresh batch to settle before claiming one reminder.
    const timer = window.setTimeout(() => {
      if (claimReminder(localStorage, Date.now())) {
        setExpanded(false)
        setVisible(true)
      }
    }, 250)
    return () => window.clearTimeout(timer)
  }, [ready, groups, visible, now])
  useEffect(() => {
    if (!visible || expanded || hovered || focused) return
    const timer = window.setTimeout(() => setVisible(false), 60_000)
    return () => window.clearTimeout(timer)
  }, [visible, expanded, hovered, focused])
  if (!visible || !groups.length) return null
  const count = groups.reduce((total, group) => total + group.credits.length, 0)
  const nextExpiry = new Date(groups[0].credits[0].expiresAt! * 1000)
  return (
    <aside className="reset-reminder" aria-label="Expiring usage resets" onMouseEnter={() => setHovered(true)} onMouseLeave={() => setHovered(false)} onFocusCapture={() => setFocused(true)} onBlurCapture={event => { if (!event.currentTarget.contains(event.relatedTarget)) setFocused(false) }}>
      <div className="reset-reminder-icon" aria-hidden="true"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d="M6 3h12M6 21h12M7 3v4c0 2 2 4 5 5-3 1-5 3-5 5v4M17 3v4c0 2-2 4-5 5 3 1 5 3 5 5v4" /></svg></div>
      <div className="reset-reminder-content">
        <div role="status" aria-live="polite" aria-atomic="true">
          <strong>Usage resets expiring</strong>
          <p>{count} {count === 1 ? "reset" : "resets"}{groups.length > 1 ? ` across ${groups.length} accounts` : ` for ${groups[0].profile.name}`} expire{count === 1 ? "s" : ""} within 48 hours.</p>
          <span className="reset-reminder-next">Next expiry: <time dateTime={nextExpiry.toISOString()}>{nextExpiry.toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })}</time></span>
        </div>
        <button className="reset-reminder-view" type="button" aria-expanded={expanded} onClick={() => setExpanded(value => !value)}>{expanded ? "Hide resets" : "View resets"}<span aria-hidden="true">{expanded ? "↑" : "→"}</span></button>
        {expanded ? <div className="reset-reminder-accounts">{groups.map(group => <button key={group.profile.id} type="button" onClick={() => { setVisible(false); onView(group) }}><span>{group.profile.name}</span><small>{group.credits.length} {group.credits.length === 1 ? "reset" : "resets"}</small><span aria-hidden="true">→</span></button>)}</div> : null}
      </div>
      <button className="reset-reminder-close" type="button" aria-label="Dismiss reset reminder" onClick={() => setVisible(false)}>×</button>
    </aside>
  )
}
