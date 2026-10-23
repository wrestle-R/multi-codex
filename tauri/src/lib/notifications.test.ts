import { expect, it } from "vitest"
import { defaultPreferences } from "./app-preferences"
import { emptyNotifications, observeAccount } from "./notifications"
import type { ProfileLimits } from "./types"
const now = Date.parse("2026-10-23T12:00:00Z")
const limits = (overrides: Partial<ProfileLimits> = {}, step = 0): ProfileLimits => ({ fiveHour: null, weekly: { remainingPercent: 60, resetsAt: now / 1000 + 3600 }, monthly: null, resetCredits: [], resetCreditsAvailable: 0, checkedAt: new Date(now + step).toISOString(), ...overrides })
const credit = (id: string) => ({ id, status: "available", resetType: "codexRateLimits", grantedAt: now / 1000, expiresAt: now / 1000 + 86400 })
const check = (state = emptyNotifications(), data = limits(), prefs = defaultPreferences) => observeAccount(state, "account", "Personal", data, prefs, now + 5000)
it("baselines existing credits, detects new IDs even at the same balance and survives restart without duplicates", () => {
  const first = check(undefined, limits({ resetCredits: [credit("old")], resetCreditsAvailable: 1 }))
  expect(first.added).toEqual([])
  const next = check(first.state, limits({ resetCredits: [credit("new")], resetCreditsAvailable: 1 }, 1000))
  expect(next.added.map(n => n.kind)).toEqual(["banked"])
  const restarted = check(JSON.parse(JSON.stringify(next.state)), limits({ resetCredits: [credit("new")] }, 2000))
  expect(restarted.added).toEqual([])
})
it("unknown credit data is not zero and a new balance is reported only from a known previous balance", () => {
  const first = check(undefined, limits({ resetCredits: null, resetCreditsAvailable: null }))
  const known = check(first.state, limits({ resetCredits: null, resetCreditsAvailable: 2 }, 1000))
  expect(known.added).toEqual([])
  const increased = check(known.state, limits({ resetCredits: null, resetCreditsAvailable: 3 }, 2000))
  expect(increased.added[0].kind).toBe("banked")
})
it("only weekly restoration generates a reset alert, not a 5-hour increase or time change alone", () => {
  const first = check(undefined, limits({ fiveHour: { remainingPercent: 10, resetsAt: null } }))
  const hourly = check(first.state, limits({ fiveHour: { remainingPercent: 100, resetsAt: null } }, 1000))
  expect(hourly.added).toEqual([])
  const weekly = check(hourly.state, limits({ weekly: { remainingPercent: 100, resetsAt: now / 1000 + 604800 } }, 2000))
  expect(weekly.added.map(n => n.kind)).toEqual(["restored"])
  const moved = check(weekly.state, limits({ weekly: { remainingPercent: 100, resetsAt: now / 1000 + 700000 } }, 3000))
  expect(moved.added).toEqual([])
})
it("detects a scheduled weekly period rolling over even if both samples show full allowance", () => {
  const first = check(undefined, limits({ weekly: { remainingPercent: 100, resetsAt: now / 1000 + 1 } }))
  const next = check(first.state, limits({ weekly: { remainingPercent: 100, resetsAt: now / 1000 + 604800 } }, 2000))
  expect(next.added.map(n => n.kind)).toEqual(["restored"])
})
it("reports the lowest newly crossed threshold once, then rearms after recovery", () => {
  const prefs = { ...defaultPreferences, thresholds: [40, 30, 20], weeklyResets: false }
  const first = check(undefined, limits(), prefs)
  const low = check(first.state, limits({ weekly: { remainingPercent: 19, resetsAt: null } }, 1000), prefs)
  expect(low.added).toHaveLength(1); expect(low.added[0].title).toContain("Below 20%")
  const lower = check(low.state, limits({ weekly: { remainingPercent: 15, resetsAt: null } }, 2000), prefs)
  expect(lower.added).toEqual([])
  const recovered = check(lower.state, limits({}, 3000), prefs)
  expect(check(recovered.state, limits({ weekly: { remainingPercent: 29, resetsAt: null } }, 4000), prefs).added[0].title).toContain("Below 30%")
})
it("muted accounts advance the baseline without replaying suppressed grants on re-enable", () => {
  const first = check()
  const muted = check(first.state, limits({ resetCredits: [credit("new")] }, 1000), { ...defaultPreferences, mutedAccounts: ["account"] })
  expect(muted.added).toEqual([])
  expect(check(muted.state, limits({ resetCredits: [credit("new")] }, 2000)).added).toEqual([])
})
it("ignores stale and out-of-order responses", () => {
  const first = check()
  expect(check(first.state, limits({ resetCredits: [credit("late")] }, -1000)).state).toBe(first.state)
  expect(observeAccount(first.state, "account", "Personal", limits({}, -31 * 60000), defaultPreferences, now).state).toBe(first.state)
})

it("notifies once per confirmed expired sign-in, survives restart and rearms only after a fresh successful check", async () => {
  const { observeSignIn } = await import("./notifications")
  const expired = { loading: false, error: "This saved profile is safe, but its Codex session expired. Open Edit and choose Sign in again before checking limits" }
  const first = observeSignIn(emptyNotifications(), "account", "Personal", expired, defaultPreferences, now)
  expect(first.added[0].kind).toBe("signIn")
  expect(first.added[0].body).toBe("Open Edit → Sign in again to reconnect this account.")
  const restarted = JSON.parse(JSON.stringify(first.state))
  expect(observeSignIn(restarted, "account", "Personal", expired, defaultPreferences, now + 1000).added).toEqual([])
  const cached = observeSignIn(restarted, "account", "Personal", { loading: false, data: limits({}, -1000) }, defaultPreferences, now + 1000)
  expect(cached.state.authentication.account.expired).toBe(true)
  const recovered = observeSignIn(restarted, "account", "Personal", { loading: false, data: limits({}, 2000) }, defaultPreferences, now + 2000)
  expect(recovered.state.authentication.account.expired).toBe(false)
  expect(observeSignIn(recovered.state, "account", "Personal", expired, defaultPreferences, now + 3000).added[0].id).not.toBe(first.added[0].id)
})
it("does not confuse network errors with expired sign-ins, and respects account and alert toggles", async () => {
  const { observeSignIn } = await import("./notifications")
  for (const error of ["Codex limits check timed out", "Codex could not reach the limits service", "429 Too many requests"]) {
    expect(observeSignIn(emptyNotifications(), "account", "Personal", { loading: false, error }, defaultPreferences, now).added).toEqual([])
  }
  for (const prefs of [{ ...defaultPreferences, signInExpired: false }, { ...defaultPreferences, mutedAccounts: ["account"] }]) {
    const result = observeSignIn(emptyNotifications(), "account", "Personal", { loading: false, error: "Codex session expired" }, prefs, now)
    expect(result.added).toEqual([])
    expect(result.state.authentication.account.expired).toBe(true)
  }
})
