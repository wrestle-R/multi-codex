import { beforeEach, describe, expect, it } from "vitest"
import { claimReminder, expiringResets, nextReminderHistory, readReminderHistory, reminderGapMs, resetReminderKey } from "./reset-reminders"
import type { Profile, ProfileLimits, ResetCredit } from "./types"

const now = new Date(2026, 9, 7, 8).getTime()
const profile: Profile = { id: "one", name: "Personal", authMode: "ChatGPT", createdAt: "", updatedAt: "", status: "idle" }
const credit = (id: string, hours: number, status = "available"): ResetCredit => ({ id, status, expiresAt: (now + hours * 60 * 60_000) / 1000, grantedAt: 0, resetType: "codexRateLimits" })
const limits = (credits: ResetCredit[]): ProfileLimits => ({ fiveHour: null, weekly: null, monthly: null, resetCreditsAvailable: credits.length, resetCredits: credits, checkedAt: new Date(now).toISOString() })

beforeEach(() => localStorage.clear())

describe("expiring usage resets", () => {
  it("groups multiple available resets across accounts and excludes expired, used, undated and distant credits", () => {
    const personal = limits([credit("a", 47), credit("b", 1), credit("expired", 0), credit("used", 1, "used"), credit("later", 49), { ...credit("undated", 1), expiresAt: null }])
    const work = limits([credit("c", 48)])
    const groups = expiringResets([profile, { ...profile, id: "two", name: "Work" }], { one: { loading: false, data: personal }, two: { loading: false, data: work } }, now)
    expect(groups.map(group => [group.profile.name, group.credits.map(item => item.id)])).toEqual([["Personal", ["b", "a"]], ["Work", ["c"]]])
  })

  it("ignores failed, refreshing, stale and zero-available snapshots, and deduplicates credit IDs", () => {
    const data = limits([credit("a", 1), credit("a", 1)])
    expect(expiringResets([profile], { one: { loading: false, data } }, now)[0].credits).toHaveLength(1)
    for (const check of [{ loading: true, data }, { loading: false, data, error: "Offline" }, { loading: false, data: { ...data, checkedAt: new Date(now - 31 * 60_000).toISOString() } }, { loading: false, data: { ...data, resetCreditsAvailable: 0 } }]) {
      expect(expiringResets([profile], { one: check }, now)).toEqual([])
    }
  })
})

describe("reminder frequency", () => {
  it("allows exactly two daily reminders spaced at least four hours apart", () => {
    const first = nextReminderHistory(null, now)!
    expect(nextReminderHistory(first, now + reminderGapMs - 1)).toBeNull()
    const second = nextReminderHistory(first, now + reminderGapMs)!
    expect(second.count).toBe(2)
    expect(nextReminderHistory(second, now + 2 * reminderGapMs)).toBeNull()
  })

  it("retains the four-hour gap across local midnight", () => {
    const late = new Date(2026, 9, 7, 23).getTime()
    const previous = nextReminderHistory(null, late)!
    expect(nextReminderHistory(previous, late + 3 * 60 * 60_000)).toBeNull()
    expect(nextReminderHistory(previous, late + reminderGapMs)?.count).toBe(1)
  })

  it("persists the limit across repeated claims and reloads, and suppresses clock rollback", () => {
    expect(claimReminder(localStorage, now)).toBe(true)
    expect(claimReminder(localStorage, now)).toBe(false)
    expect(nextReminderHistory(readReminderHistory(localStorage), now - 1)).toBeNull()
    expect(claimReminder(localStorage, now + reminderGapMs)).toBe(true)
    expect(claimReminder(localStorage, now + 2 * reminderGapMs)).toBe(false)
    expect(JSON.parse(localStorage.getItem(resetReminderKey)!)).toMatchObject({ count: 2 })
  })

  it("handles invalid saved state and avoids repeated popups when storage cannot persist the cap", () => {
    localStorage.setItem(resetReminderKey, '{"day":"today","count":-1,"lastShownAt":0}')
    expect(readReminderHistory(localStorage)).toBeNull()
    expect(claimReminder({ getItem: () => null, setItem: () => { throw new Error("Storage blocked") } }, now)).toBe(false)
  })
})
