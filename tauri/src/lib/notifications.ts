import type { AppPreferences } from "./app-preferences"
import type { LimitCheckState, ProfileLimits } from "./types"

export interface AppNotification { id: string; title: string; body: string; accountId?: string; createdAt: number; read: boolean; kind: "banked" | "restored" | "low" | "update" | "signIn" }
export interface AccountObservation { limits: ProfileLimits; creditIds: string[] }
export interface NotificationState { authentication: Record<string, { expired: boolean; episode: number; observedAt?: number }>; accounts: Record<string, AccountObservation>; receipts: string[]; items: AppNotification[] }
export const emptyNotifications = (): NotificationState => ({ authentication: {}, accounts: {}, receipts: [], items: [] })
export const notificationKey = `multi-codex-notifications:v1:${window.__TAURI_INTERNALS__ && new URLSearchParams(location.search).get("demo") !== "1" ? "native" : "preview"}`
export function readNotifications(): NotificationState {
  try {
    const value = JSON.parse(localStorage.getItem(notificationKey) ?? "null") as NotificationState | null
    if (value && value.accounts && Array.isArray(value.receipts) && Array.isArray(value.items)) {
      const authentication = Object.fromEntries(Object.entries(value.authentication ?? {}).filter(([, observation]) => observation && typeof observation.expired === "boolean" && Number.isInteger(observation.episode) && observation.episode >= 0))
      return { authentication, accounts: Object.fromEntries(Object.entries(value.accounts).filter(([, a]) => a?.limits && Number.isFinite(Date.parse(a.limits.checkedAt)) && Array.isArray(a.creditIds))), receipts: value.receipts.filter(id => typeof id === "string").slice(-2048), items: value.items.filter(item => item && typeof item.id === "string" && typeof item.title === "string" && typeof item.body === "string" && Number.isFinite(item.createdAt)).slice(0, 100) }
    }
  } catch { /* A damaged history must not interrupt account checks. */ }
  return emptyNotifications()
}
export function observeAccount(state: NotificationState, id: string, name: string, limits: ProfileLimits, prefs: AppPreferences, now: number): { state: NotificationState; added: AppNotification[] } {
  const previous = state.accounts[id]
  const checked = Date.parse(limits.checkedAt)
  // Only successful, fresh, newer responses can generate events, never cached/loading/error data.
  if (!Number.isFinite(checked) || now - checked > 30 * 60_000 || checked > now + 60_000 || (previous && checked <= Date.parse(previous.limits.checkedAt))) return { state, added: [] }
  const added: AppNotification[] = []
  const receipts = new Set(state.receipts)
  const enabled = prefs.notifications && !prefs.mutedAccounts.includes(id)
  const add = (eventId: string, kind: AppNotification["kind"], title: string, body: string, selected: boolean) => {
    if (receipts.has(eventId)) return
    receipts.add(eventId)
    if (enabled && selected) added.push({ id: eventId, kind, title, body, accountId: id, createdAt: now, read: false })
  }
  const credits = limits.resetCredits?.filter(c => c.status.toLowerCase() === "available" && (c.expiresAt == null || c.expiresAt * 1000 > now)) ?? []
  if (previous) {
    const newCredits = credits.filter(c => !previous.creditIds.includes(c.id))
    // IDs detect a new grant even if another reset was redeemed in the same interval.
    if (newCredits.length && previous.limits.resetCredits != null) {
      for (const credit of newCredits) add(`banked:${id}:${credit.id}`, "banked", `${name} · New banked reset`, "A new reset was added to this account. It is saved for later use. Receiving a banked reset does not itself restore usage.", prefs.bankedResets)
    } else if ((limits.resetCredits == null || previous.limits.resetCredits == null) && previous.limits.resetCreditsAvailable != null && limits.resetCreditsAvailable != null && limits.resetCreditsAvailable > previous.limits.resetCreditsAvailable) {
      add(`balance:${id}:${limits.checkedAt}`, "banked", `${name} · Banked resets increased`, `${limits.resetCreditsAvailable} banked resets are now available on this account.`, prefs.bankedResets)
    }
  }
  for (const [key, label] of [["fiveHour", "5-hour"], ["weekly", "Weekly"], ["monthly", "Monthly"]] as const) {
    const window = limits[key], old = previous?.limits[key]
    if (!window || !Number.isFinite(window.remainingPercent) || window.remainingPercent < 0 || window.remainingPercent > 100) continue
    const crossed = prefs.thresholds.filter(t => window.remainingPercent < t && (!old || old.remainingPercent >= t))
    if (crossed.length) {
      const threshold = Math.min(...crossed)
      add(`low:${id}:${key}:${limits.checkedAt}`, "low", `${name} · Below ${threshold}%`, `${label} allowance has ${Math.floor(window.remainingPercent)}% remaining.`, true)
    }
  }
  const weekly = limits.weekly, oldWeekly = previous?.limits.weekly
  if (weekly && oldWeekly && Number.isFinite(weekly.remainingPercent) && (
    weekly.remainingPercent > oldWeekly.remainingPercent ||
    (weekly.resetsAt != null && oldWeekly.resetsAt != null && weekly.resetsAt > oldWeekly.resetsAt && oldWeekly.resetsAt * 1000 <= checked)
  )) add(`weekly:${id}:${limits.checkedAt}`, "restored", `${name} · Weekly usage reset`, `A refreshed weekly allowance was detected. ${Math.round(weekly.remainingPercent)}% is now remaining.`, prefs.weeklyResets)
  const creditIds = [...new Set([...(previous?.creditIds ?? []), ...credits.map(c => c.id)])].slice(-512)
  const next = { ...state, accounts: { ...state.accounts, [id]: { limits, creditIds } }, receipts: [...receipts].slice(-2048), items: [...added].reverse().concat(state.items).slice(0, 100) }
  return { state: next, added }
}

export function observeSignIn(state: NotificationState, id: string, name: string, check: LimitCheckState, prefs: AppPreferences, now: number): { state: NotificationState; added: AppNotification[] } {
  if (check.loading) return { state, added: [] }
  // Match the core's sanitised expired-session result. Network/service failures
  // do not prove an expired sign-in and never produce this notification.
  const expired = /\bCodex session expired\b/i.test(check.error ?? "")
  const old = state.authentication[id]
  const fresh = check.data && Date.parse(check.data.checkedAt) >= now - 30 * 60_000 && Date.parse(check.data.checkedAt) <= now + 60_000
  if (!expired) {
    if (!check.error && fresh && old?.expired && Date.parse(check.data!.checkedAt) > (old.observedAt ?? 0)) return { state: { ...state, authentication: { ...state.authentication, [id]: { ...old, expired: false } } }, added: [] }
    return { state, added: [] }
  }
  if (old?.expired) return { state, added: [] }
  const episode = (old?.episode ?? 0) + 1
  const item: AppNotification = { id: `sign-in:${id}:${episode}`, accountId: id, kind: "signIn", title: `${name} · Sign-in expired`, body: "Open Edit → Sign in again to reconnect this account.", createdAt: now, read: false }
  const enabled = prefs.notifications && prefs.signInExpired && !prefs.mutedAccounts.includes(id)
  const added = enabled ? [item] : []
  return { state: { ...state, authentication: { ...state.authentication, [id]: { expired: true, episode, observedAt: now } }, items: [...added, ...state.items].slice(0, 100) }, added }
}
