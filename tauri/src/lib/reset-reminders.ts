import type { LimitCheckState, Profile, ProfileLimits, ResetCredit } from "./types"

export const reminderWindowMs = 48 * 60 * 60_000
export const reminderGapMs = 4 * 60 * 60_000
const freshnessMs = 30 * 60_000
export const resetReminderKey = typeof window !== "undefined" && window.__TAURI_INTERNALS__ && new URLSearchParams(window.location.search).get("demo") !== "1"
  ? "multi-codex-reset-reminders:v1:native" : "multi-codex-reset-reminders:v1:preview"

export interface ExpiringResetGroup {
  profile: Profile
  limits: ProfileLimits
  credits: ResetCredit[]
}
export interface ReminderHistory {
  day: string
  count: number
  lastShownAt: number
}

export function expiringResets(profiles: Profile[], checks: Record<string, LimitCheckState>, now: number): ExpiringResetGroup[] {
  return profiles.flatMap(profile => {
    const check = checks[profile.id]
    if (profile.authMode.toLowerCase() !== "chatgpt" || !check?.data || check.loading || check.error) return []
    const limits = check.data
    const age = now - Date.parse(limits.checkedAt)
    if (!Number.isFinite(age) || age < 0 || age > freshnessMs || (limits.resetCreditsAvailable ?? 0) <= 0) return []
    const seen = new Set<string>()
    const credits = (limits.resetCredits ?? []).filter(credit => {
      if (seen.has(credit.id) || credit.status?.toLowerCase() !== "available" || credit.expiresAt == null || !Number.isFinite(credit.expiresAt)) return false
      seen.add(credit.id)
      const remaining = credit.expiresAt * 1000 - now
      return remaining > 0 && remaining <= reminderWindowMs
    }).sort((a, b) => a.expiresAt! - b.expiresAt!).slice(0, limits.resetCreditsAvailable!)
    return credits.length ? [{ profile, limits, credits }] : []
  }).sort((a, b) => a.credits[0].expiresAt! - b.credits[0].expiresAt!)
}

function localDay(now: number): string {
  const date = new Date(now)
  return `${date.getFullYear()}-${date.getMonth() + 1}-${date.getDate()}`
}

export function readReminderHistory(storage: Pick<Storage, "getItem">): ReminderHistory | null {
  try {
    const value: unknown = JSON.parse(storage.getItem(resetReminderKey) ?? "null")
    if (!value || typeof value !== "object") return null
    const history = value as ReminderHistory
    return typeof history.day === "string" && Number.isInteger(history.count) && history.count >= 0 && Number.isFinite(history.lastShownAt) && history.lastShownAt >= 0 ? history : null
  } catch { return null }
}

export function nextReminderHistory(previous: ReminderHistory | null, now: number): ReminderHistory | null {
  if (previous && now - previous.lastShownAt < reminderGapMs) return null
  const day = localDay(now)
  const count = previous?.day === day ? previous.count : 0
  return count >= 2 ? null : { day, count: count + 1, lastShownAt: now }
}

export function claimReminder(storage: Pick<Storage, "getItem" | "setItem">, now: number): boolean {
  const next = nextReminderHistory(readReminderHistory(storage), now)
  if (!next) return false
  try {
    // Persist before showing so dismissal, reload and restart retain the limit.
    storage.setItem(resetReminderKey, JSON.stringify(next))
    return true
  } catch { return false }
}
