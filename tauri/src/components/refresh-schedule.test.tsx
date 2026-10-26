import { act, cleanup, renderHook } from "@testing-library/react"
import { afterEach, expect, it, vi } from "vitest"
import { useProfileLimits } from "./use-profile-limits"
import { useAppUpdate } from "./use-app-update"
import type { Profile } from "../lib/types"
const api = vi.hoisted(() => ({ limits: vi.fn(), update: vi.fn() }))
vi.mock("../lib/desktop-api", () => ({ checkProfileLimits: api.limits, checkAppUpdate: api.update }))
const profiles: Profile[] = [{ id: "refresh-test", name: "Test", authMode: "chatgpt", status: "idle", createdAt: "", updatedAt: "" }]
afterEach(() => { cleanup(); vi.useRealTimers(); api.limits.mockReset(); api.update.mockReset(); localStorage.clear() })
it("defaults to ten minutes for usage and update checks, then replaces both timers when changed", async () => {
  vi.useFakeTimers()
  api.limits.mockResolvedValue({ checkedAt: new Date().toISOString(), fiveHour: null, weekly: null, monthly: null, resetCredits: null, resetCreditsAvailable: null })
  api.update.mockResolvedValue(null)
  const { rerender } = renderHook(({ minutes }: { minutes?: number }) => { useProfileLimits(profiles, true, minutes); useAppUpdate(false, minutes) }, { initialProps: {} as {minutes?: number} })
  await act(async () => {})
  expect(api.limits).toHaveBeenCalledTimes(1); expect(api.update).toHaveBeenCalledTimes(1)
  await act(async () => { await vi.advanceTimersByTimeAsync(9 * 60_000) })
  expect(api.limits).toHaveBeenCalledTimes(1); expect(api.update).toHaveBeenCalledTimes(1)
  await act(async () => { await vi.advanceTimersByTimeAsync(60_000) })
  expect(api.limits).toHaveBeenCalledTimes(2); expect(api.update).toHaveBeenCalledTimes(2)
  rerender({ minutes: 1 })
  await act(async () => { await vi.advanceTimersByTimeAsync(60_000) })
  expect(api.limits).toHaveBeenCalledTimes(3); expect(api.update).toHaveBeenCalledTimes(4)
})
it("coalesces slow account checks across refresh ticks instead of stacking requests", async () => {
  vi.useFakeTimers(); api.limits.mockReturnValue(new Promise(() => {}))
  const { result } = renderHook(() => useProfileLimits(profiles, true, 1))
  await act(async () => { await vi.advanceTimersByTimeAsync(3 * 60_000) })
  expect(api.limits).toHaveBeenCalledTimes(1)
  expect(result.current.refreshingAllLimits).toBe(true)
})
