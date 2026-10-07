import type { LimitCheckState, ProfileLimits } from "./types"

// Preview data stays separate from the native app's account snapshots.
export const limitsCacheKey = typeof window !== "undefined" && window.__TAURI_INTERNALS__ && new URLSearchParams(window.location.search).get("demo") !== "1"
  ? "multi-codex-limits:v1:native" : "multi-codex-limits:v1:preview"

export function readLimitsCache(): Record<string, LimitCheckState> {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(limitsCacheKey) ?? "{}")
    if (!saved || typeof saved !== "object" || Array.isArray(saved)) return {}
    return Object.fromEntries(Object.entries(saved).filter(([, value]) => {
      if (!value || typeof value !== "object") return false
      const data = value as ProfileLimits
      return typeof data.checkedAt === "string" && Number.isFinite(Date.parse(data.checkedAt))
        && [data.fiveHour, data.weekly, data.monthly].every(window => window === null || (
          window && Number.isFinite(window.remainingPercent) && window.remainingPercent >= 0 && window.remainingPercent <= 100
          && (window.resetsAt === null || Number.isFinite(window.resetsAt))
        ))
        && (data.resetCreditsAvailable === null || Number.isFinite(data.resetCreditsAvailable))
        && (data.resetCredits === null || Array.isArray(data.resetCredits))
    }).map(([id, data]) => [id, { loading: false, data: data as ProfileLimits }]))
  } catch { return {} }
}

export function saveLimitsCache(checks: Record<string, LimitCheckState>, ids: Set<string>) {
  try {
    localStorage.setItem(limitsCacheKey, JSON.stringify(Object.fromEntries(
      Object.entries(checks).filter(([id, check]) => ids.has(id) && check.data).map(([id, check]) => [id, check.data]),
    )))
  } catch { /* A full or unavailable storage area must not stop live refresh. */ }
}
