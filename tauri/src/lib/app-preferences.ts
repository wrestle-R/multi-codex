import { useSyncExternalStore } from "react"
export const refreshIntervals = [1, 5, 10, 15, 30, 60] as const
export interface AppPreferences {
  refreshMinutes: number
  notifications: boolean
  desktopNotifications: boolean
  updates: boolean
  bankedResets: boolean
  weeklyResets: boolean
  expiringResets: boolean
  signInExpired: boolean
  thresholds: number[]
  mutedAccounts: string[]
  seasonalAvatars: boolean
}
export const defaultPreferences: AppPreferences = { refreshMinutes: 10, notifications: true, desktopNotifications: false, updates: true, bankedResets: true, weeklyResets: true, expiringResets: true, signInExpired: true, thresholds: [20], mutedAccounts: [], seasonalAvatars: true }
const key = "multi-codex-preferences:v1"
let cached: AppPreferences | undefined
function read(): AppPreferences {
  if (cached) return cached
  let value: Partial<AppPreferences> = {}
  try { value = JSON.parse(localStorage.getItem(key) ?? "{}") ?? {} } catch { /* Defaults survive damaged storage. */ }
  const next = { ...defaultPreferences }
  for (const field of ["notifications", "desktopNotifications", "updates", "bankedResets", "weeklyResets", "expiringResets", "signInExpired", "seasonalAvatars"] as const) if (typeof value[field] === "boolean") next[field] = value[field]
  if (refreshIntervals.includes(value.refreshMinutes as typeof refreshIntervals[number])) next.refreshMinutes = value.refreshMinutes!
  for (const field of ["mutedAccounts"] as const) if (Array.isArray(value[field])) next[field] = value[field].filter((id): id is string => typeof id === "string")
  if (Array.isArray(value.thresholds)) next.thresholds = [...new Set(value.thresholds.filter(n => [20, 30, 40].includes(n)))].sort((a, b) => b - a)
  return cached = next
}
function subscribe(callback: () => void) {
  const update = (event: Event) => {
    if (event instanceof StorageEvent && event.key !== null && event.key !== key) return
    cached = undefined; callback()
  }
  window.addEventListener("storage", update)
  window.addEventListener("multi-codex-preferences", update)
  return () => { window.removeEventListener("storage", update); window.removeEventListener("multi-codex-preferences", update) }
}
export function savePreferences(next: AppPreferences) {
  localStorage.setItem(key, JSON.stringify(next))
  cached = undefined
  window.dispatchEvent(new Event("multi-codex-preferences"))
}
export function useAppPreferences() { return useSyncExternalStore(subscribe, read) }
