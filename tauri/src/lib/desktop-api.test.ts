import { afterEach, beforeEach, expect, it, vi } from "vitest"

const invoke = vi.hoisted(() => vi.fn())
vi.mock("@tauri-apps/api/core", () => ({ invoke }))

beforeEach(() => {
  vi.resetModules()
  invoke.mockReset()
  vi.stubGlobal("__TAURI_INTERNALS__", {})
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

it("uses the native opener when macOS handles browser launch", async () => {
  invoke.mockResolvedValue(true)
  const open = vi.spyOn(window, "open").mockReturnValue(null)
  const { openDeviceLoginBrowser } = await import("./desktop-api")
  await openDeviceLoginBrowser()
  expect(invoke).toHaveBeenCalledWith("open_device_login_browser")
  expect(open).not.toHaveBeenCalled()
})

it("preserves the existing browser action on Linux", async () => {
  invoke.mockResolvedValue(false)
  const open = vi.spyOn(window, "open").mockReturnValue(null)
  const { openDeviceLoginBrowser } = await import("./desktop-api")
  await openDeviceLoginBrowser()
  expect(open).toHaveBeenCalledWith("https://auth.openai.com/codex/device", "_blank", "noopener,noreferrer")
})

it("propagates native opener failures so the dialog can offer a retry", async () => {
  invoke.mockRejectedValue("Could not open the default browser")
  const open = vi.spyOn(window, "open").mockReturnValue(null)
  const { openDeviceLoginBrowser } = await import("./desktop-api")
  await expect(openDeviceLoginBrowser()).rejects.toBe("Could not open the default browser")
  expect(open).not.toHaveBeenCalled()
})

it("requests current native launch targets rather than trusting saved detection", async () => {
  const targets = { platform: "macos", vscodeInstalled: true, codexCliAvailable: true, standaloneInstalled: false, standaloneVerified: false }
  invoke.mockResolvedValue(targets)
  const { getLaunchTargets } = await import("./desktop-api")
  expect(await getLaunchTargets()).toEqual(targets)
  expect(invoke).toHaveBeenCalledWith("get_launch_targets")
})
