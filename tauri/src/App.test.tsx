import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import App, { formatStorage } from "./App"
import type { Profile } from "./lib/types"

const api = vi.hoisted(() => ({
  getDesktopInventory: vi.fn(),
  discardPlacement: vi.fn(),
  getExecutableSettings: vi.fn(),
  saveExecutableSettings: vi.fn(),
  listProfiles: vi.fn(),
  addProfile: vi.fn(),
  beginDeviceLogin: vi.fn(),
  cancelDeviceLogin: vi.fn(),
  listWorkspaceDirectories: vi.fn(),
  clearProfileCache: vi.fn(),
  importCurrentProfile: vi.fn(),
  updateProfile: vi.fn(),
  checkProfileLimits: vi.fn(),
  subscribeDeviceLogin: vi.fn(),
  launchProfile: vi.fn(),
  deleteProfile: vi.fn(),
  getDesktopIntegrationStatus: vi.fn(),
  getLaunchEnvironment: vi.fn(),
  getStorageUsage: vi.fn(),
  installDesktopIntegration: vi.fn(),
}))

vi.mock("./lib/desktop-api", () => api)

const profile: Profile = {
  id: "2c7f23ba-b2c0-4f67-a963-7749cc13f1e2",
  name: "Personal",
  authMode: "ChatGPT",
  createdAt: "2026-09-01T09:30:00Z",
  updatedAt: "2026-09-04T08:10:00Z",
  status: "idle",
}

let deviceLoginListener: ((event: { id: string; output?: string; completed: boolean; error?: string }) => void) | undefined

beforeEach(() => {
  localStorage.clear()
  api.listProfiles.mockReset().mockResolvedValue([profile])
  api.addProfile.mockReset().mockResolvedValue(profile)
  api.beginDeviceLogin.mockReset().mockResolvedValue("device-login")
  api.cancelDeviceLogin.mockReset().mockResolvedValue(undefined)
  api.listWorkspaceDirectories.mockReset().mockImplementation(async (path: string) => ({ path, parentPath: "/home/rdp/Desktop", directories: [] }))
  api.clearProfileCache.mockReset().mockResolvedValue(256_000_000)
  api.importCurrentProfile.mockReset().mockResolvedValue(profile)
  api.updateProfile.mockReset().mockResolvedValue(profile)
  api.checkProfileLimits.mockReset().mockResolvedValue({
    fiveHour: { remainingPercent: 76, resetsAt: 1788597000 },
    weekly: { remainingPercent: 43, resetsAt: 1788998400 },
    monthly: null,
    resetCreditsAvailable: 2,
    resetCredits: [
      { id: "credit-1", status: "available", resetType: "codexRateLimits", grantedAt: 1788500000, expiresAt: 1893456000, title: "Usage reset" },
    ],
    checkedAt: "2026-09-05T04:30:00Z",
  })
  api.launchProfile.mockReset().mockResolvedValue({ completed: true, error: null, retryToken: null })
  api.discardPlacement.mockReset().mockResolvedValue(undefined)
  api.getExecutableSettings.mockReset().mockResolvedValue({ codePath: null, codexPath: null, globalCodexHome: null })
  api.saveExecutableSettings.mockReset().mockImplementation(async settings => settings)
  api.getDesktopInventory.mockReset().mockResolvedValue({
    protocolVersion: 1,
    capabilities: { backend: "hyprland", enumerateDesktops: true, enumerateWindows: true, moveWindows: true, reason: null },
    desktops: [
      { id: "hyprland:7", name: "Desktop 7", monitor: "Main display", current: false, windows: [{ id: "window", pid: 42, application: "Visual Studio Code", title: "Project — Code" }] },
      { id: "hyprland:12", name: "Desktop 12", monitor: "Other display", current: true, windows: [] },
    ],
  })
  deviceLoginListener = undefined
  api.subscribeDeviceLogin.mockReset().mockImplementation(async (listener) => {
    deviceLoginListener = listener
    return () => undefined
  })
  api.deleteProfile.mockReset().mockResolvedValue(undefined)
  api.getDesktopIntegrationStatus.mockReset().mockResolvedValue({
    available: false,
    installed: true,
    desktopShortcut: false,
    version: "0.2.0",
    source: "package",
  })
  api.getStorageUsage.mockReset().mockResolvedValue({
    bytes: 9_876_543_210,
    reclaimableBytes: 256_000_000,
    otherBytes: 1_024,
    profiles: [{ id: profile.id, name: profile.name, bytes: 9_876_542_186, reclaimableBytes: 256_000_000, running: false }],
  })
  api.getLaunchEnvironment.mockReset().mockResolvedValue({ defaultWorkspace: "/home/rdp/Desktop/code", capabilities: { backend: "hyprland", enumerateDesktops: true, enumerateWindows: true, moveWindows: true, reason: null } })
  api.installDesktopIntegration.mockReset().mockResolvedValue({
    available: true,
    installed: true,
    desktopShortcut: true,
    version: "0.2.0",
    source: "appimage",
  })
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: vi.fn().mockImplementation(() => ({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  })
})

afterEach(() => cleanup())

describe("Multi Codex", () => {
  it("formats storage using compact binary units", () => {
    expect(formatStorage(0)).toBe("0 B")
    expect(formatStorage(9_876_543_210)).toBe("9.2 GB")
  })

  it("loads profiles and exposes profile actions", async () => {
    render(<App />)
    expect(await screen.findByRole("heading", { name: "Personal" })).toBeInTheDocument()
    expect(screen.getByRole("button", { name: "Edit Personal" })).toBeEnabled()
    expect(screen.getByRole("button", { name: "Delete Personal" })).toBeEnabled()
    expect(screen.getByRole("button", { name: "Launch" })).toBeEnabled()
  })

  it("opens the in-app folder picker at the preferred folder before the desktop chooser", async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Launch" }))
    expect(await screen.findByRole("dialog", { name: "Choose a folder" })).toBeInTheDocument()
    expect(api.listWorkspaceDirectories).toHaveBeenCalledWith("/home/rdp/Desktop/code")
    await user.click(screen.getByRole("button", { name: "Choose this folder" }))
    await user.click(await screen.findByRole("button", { name: /Current desktop/ }))
    await waitFor(() => expect(api.launchProfile).toHaveBeenCalledWith(profile.id, "/home/rdp/Desktop/code", null, null))
  })

  it("forwards a stable Hyprland desktop identifier after folder selection", async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Launch" }))
    await user.click(await screen.findByRole("button", { name: "Choose this folder" }))
    await user.click(await screen.findByRole("button", { name: "Open on Desktop 7" }))
    await waitFor(() => expect(api.launchProfile).toHaveBeenCalledWith(profile.id, "/home/rdp/Desktop/code", "hyprland:7", null))
  })

  it("shows ten desktops with grouped application icons and returns to accounts after placement", async () => {
    api.getDesktopInventory.mockResolvedValue({
      protocolVersion: 1,
      capabilities: { backend: "hyprland", enumerateDesktops: true, enumerateWindows: true, moveWindows: true, reason: null },
      desktops: Array.from({ length: 10 }, (_, index) => ({
        id: `hyprland:${index + 1}`, name: `Desktop ${index + 1}`, monitor: "Main display", current: index === 5,
        windows: index === 3 ? [
          { id: "code-one", pid: 42, application: "Visual Studio Code", title: "First project", icon: "data:image/png;base64,test" },
          { id: "code-two", pid: 42, application: "Visual Studio Code", title: "Second project", icon: "data:image/png;base64,test" },
        ] : [],
      })),
    })
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Launch" }))
    await user.click(await screen.findByRole("button", { name: "Choose this folder" }))
    const grid = await screen.findByLabelText("Available desktops")
    await waitFor(() => expect(within(grid).getAllByRole("button")).toHaveLength(10))
    const fourth = within(grid).getByRole("button", { name: "Open on Desktop 4" })
    expect(fourth).toHaveTextContent("2 windows")
    expect(within(fourth).getAllByRole("img", { name: "Visual Studio Code" })).toHaveLength(1)
    expect(await screen.findByText("First project")).toBeVisible()
    fireEvent.error(within(fourth).getByRole("img"))
    expect(within(fourth).getByRole("img")).toHaveTextContent("V")
    await user.click(within(grid).getByRole("button", { name: "Open on Desktop 10" }))
    await waitFor(() => expect(api.launchProfile).toHaveBeenCalledWith(profile.id, "/home/rdp/Desktop/code", "hyprland:10", null))
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Choose a desktop" })).not.toBeInTheDocument())
    expect(screen.getByRole("heading", { name: "Personal" })).toBeVisible()
  })

  it("shows window titles and empty desktops, keeping placement failures retryable", async () => {
    const user = userEvent.setup()
    api.launchProfile.mockResolvedValueOnce({ completed: false, error: "Permission denied", retryToken: "placement-1" })
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Launch" }))
    await user.click(await screen.findByRole("button", { name: "Choose this folder" }))
    expect(await screen.findByText("Project — Code")).toBeInTheDocument()
    expect(screen.getByText("No open windows")).toBeInTheDocument()
    await user.click(screen.getByRole("button", { name: "Open on Desktop 7" }))
    expect(await screen.findByRole("alert")).toHaveTextContent("Permission denied")
    expect(screen.getByRole("dialog", { name: "Choose a desktop" })).toBeInTheDocument()
    api.getDesktopInventory.mockResolvedValue({
      protocolVersion: 1,
      capabilities: { backend: "hyprland", enumerateDesktops: true, enumerateWindows: true, moveWindows: true, reason: null },
      desktops: [{ id: "hyprland:12", name: "Desktop 12", monitor: "Other display", current: true, windows: [] }],
    })
    await user.click(screen.getByRole("button", { name: "Refresh desktops" }))
    expect(await screen.findByText("The selected desktop disappeared. Choose another destination.")).toBeInTheDocument()
    expect(screen.queryByRole("button", { name: "Open on Desktop 7" })).not.toBeInTheDocument()
    await user.click(screen.getByRole("button", { name: "Open on Desktop 12" }))
    await waitFor(() => expect(api.launchProfile).toHaveBeenLastCalledWith(profile.id, "/home/rdp/Desktop/code", "hyprland:12", "placement-1"))
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Choose a desktop" })).not.toBeInTheDocument())
  })

  it("keeps the dialog open and disables actions while a launch is pending", async () => {
    const user = userEvent.setup()
    let finish: ((value: { completed: boolean; error: null; retryToken: null }) => void) | undefined
    api.launchProfile.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Launch" }))
    await user.click(await screen.findByRole("button", { name: "Choose this folder" }))
    await user.click(await screen.findByRole("button", { name: "Open on Desktop 7" }))
    expect(screen.getByRole("dialog", { name: "Choose a desktop" })).toBeInTheDocument()
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled()
    expect(screen.getByRole("button", { name: "Open on Desktop 7" })).toBeDisabled()
    finish?.({ completed: true, error: null, retryToken: null })
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Choose a desktop" })).not.toBeInTheDocument())
  })

  it("explains unavailable macOS placement and retains current-desktop launch", async () => {
    const user = userEvent.setup()
    api.getDesktopInventory.mockResolvedValue({ protocolVersion: 1, capabilities: { backend: "macos", enumerateDesktops: false, enumerateWindows: false, moveWindows: false, reason: "Native Spaces validation is pending" }, desktops: [] })
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Launch" }))
    await user.click(await screen.findByRole("button", { name: "Choose this folder" }))
    expect(await screen.findByText("Native Spaces validation is pending")).toBeInTheDocument()
    await user.click(screen.getByRole("button", { name: /Current desktop/ }))
    await waitFor(() => expect(api.launchProfile).toHaveBeenCalledWith(profile.id, "/home/rdp/Desktop/code", null, null))
  })

  it("saves explicit tool paths through launch settings", async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole("button", { name: "Launch settings" }))
    const input = await screen.findByLabelText("VS Code executable")
    await waitFor(() => expect(input).toBeEnabled())
    await user.type(input, "/Applications/Custom Code.app/bin/code")
    await user.click(screen.getByRole("button", { name: "Save paths" }))
    await waitFor(() => expect(api.saveExecutableSettings).toHaveBeenCalledWith({ codePath: "/Applications/Custom Code.app/bin/code", codexPath: null, globalCodexHome: null }))
  })

  it("navigates folders in the in-app picker and uses the selected directory", async () => {
    const user = userEvent.setup()
    api.listWorkspaceDirectories.mockImplementation(async (path: string) => ({
      path,
      parentPath: "/home/rdp/Desktop/code",
      directories: path.endsWith("/code") ? [{ name: "sample-project", path: `${path}/sample-project` }] : [],
    }))
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Launch" }))
    await user.click(await screen.findByRole("button", { name: /sample-project/ }))
    expect(await screen.findByText("No subfolders here. You can choose this folder.")).toBeInTheDocument()
    await user.click(screen.getByRole("button", { name: "Choose this folder" }))
    await user.click(await screen.findByRole("button", { name: /Current desktop/ }))
    await waitFor(() => expect(api.launchProfile).toHaveBeenCalledWith(profile.id, "/home/rdp/Desktop/code/sample-project", null, null))
  })

  it("cancels folder selection without launching or showing the desktop chooser", async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Launch" }))
    await user.click(await screen.findByRole("button", { name: "Cancel" }))
    expect(api.launchProfile).not.toHaveBeenCalled()
    expect(screen.queryByRole("dialog", { name: "Choose a folder" })).not.toBeInTheDocument()
    expect(screen.queryByRole("dialog", { name: "Choose a desktop" })).not.toBeInTheDocument()
  })

  it("starts browser device-code sign-in without asking for credential JSON", async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Add account" }))
    await user.type(screen.getByLabelText("Profile name"), "Work")
    expect(screen.queryByLabelText("Auth JSON")).not.toBeInTheDocument()
    await user.click(screen.getByRole("button", { name: "Get sign-in code" }))
    await waitFor(() => expect(api.beginDeviceLogin).toHaveBeenCalledWith("Work", { notes: undefined }, undefined))
  })

  it("reconnects an existing profile without replacing its profile identity", async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Edit Personal" }))
    await user.click(screen.getByRole("button", { name: "Sign in again" }))
    await waitFor(() => expect(api.beginDeviceLogin).toHaveBeenCalledWith(
      "Personal",
      { notes: undefined },
      profile.id,
    ))
  })

  it("formats device sign-in details and cancels the pending login from the close button", async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Add account" }))
    await user.type(screen.getByLabelText("Profile name"), "Work")
    await user.click(screen.getByRole("button", { name: "Get sign-in code" }))
    await waitFor(() => expect(api.beginDeviceLogin).toHaveBeenCalled())
    deviceLoginListener?.({
      id: "device-login",
      output: "\u001b[90mOpen https://auth.openai.com/codex/device\u001b[0m\nCode: \u001b[94mCGNT-02M3R\u001b[0m",
      completed: false,
    })
    expect(await screen.findByRole("button", { name: "Copy link" })).toBeEnabled()
    expect(screen.getByText("CGNT-02M3R")).toBeInTheDocument()
    await user.click(screen.getByRole("button", { name: "Close" }))
    await waitFor(() => expect(api.cancelDeviceLogin).toHaveBeenCalledWith("device-login"))
    expect(screen.queryByRole("dialog", { name: "Add account" })).not.toBeInTheDocument()
  })

  it("shows saved notes without obsolete manual usage fields", async () => {
    api.listProfiles.mockResolvedValue([{ ...profile, notes: "Use for personal work" }])
    render(<App />)
    const row = await screen.findByTestId(`profile-${profile.id}`)
    expect(row).toHaveTextContent("Use for personal work")
    expect(row).not.toHaveTextContent("requests left")
  })

  it("requires both a profile name and pasted JSON", async () => {
    const user = userEvent.setup()
    render(<App />)
    await screen.findByRole("heading", { name: "Personal" })
    await user.click(screen.getByRole("button", { name: "Add account" }))
    await user.click(screen.getByRole("button", { name: "Paste JSON" }))
    const submit = within(screen.getByRole("dialog")).getByRole("button", { name: "Add account" })
    expect(submit).toBeDisabled()
    await user.type(screen.getByLabelText("Profile name"), "Work")
    expect(submit).toBeDisabled()
    await user.type(screen.getByLabelText("Auth JSON"), "{{}")
    expect(submit).toBeEnabled()
  })

  it("saves pasted credentials through the native boundary", async () => {
    const user = userEvent.setup()
    render(<App />)
    await screen.findByRole("heading", { name: "Personal" })
    await user.click(screen.getByRole("button", { name: "Add account" }))
    await user.click(screen.getByRole("button", { name: "Paste JSON" }))
    await user.type(screen.getByLabelText("Profile name"), "Work")
    fireEvent.change(screen.getByLabelText("Auth JSON"), { target: { value: '{"auth_mode":"chatgpt"}' } })
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Add account" }))
    await waitFor(() => expect(api.addProfile).toHaveBeenCalledWith({
      name: "Work",
      authJson: '{"auth_mode":"chatgpt"}',
      notes: undefined,
    }))
  })

  it("imports the current account without requesting credential text", async () => {
    const user = userEvent.setup()
    render(<App />)
    await screen.findByRole("heading", { name: "Personal" })
    await user.click(screen.getByRole("button", { name: "Add account" }))
    await user.click(screen.getByRole("button", { name: "Import current" }))
    await user.type(screen.getByLabelText("Profile name"), "Current")
    expect(screen.queryByLabelText("Auth JSON")).not.toBeInTheDocument()
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Add account" }))
    await waitFor(() => expect(api.importCurrentProfile).toHaveBeenCalledWith("Current", {
      notes: undefined,
    }))
  })

  it("saves optional notes without manual usage inputs", async () => {
    const user = userEvent.setup()
    render(<App />)
    await screen.findByRole("heading", { name: "Personal" })
    await user.click(screen.getByRole("button", { name: "Edit Personal" }))
    expect(screen.queryByLabelText(/Requests remaining/)).not.toBeInTheDocument()
    expect(screen.queryByLabelText(/Reset date/)).not.toBeInTheDocument()
    await user.type(screen.getByLabelText(/Notes/), "Use for personal work")
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Save changes" }))
    await waitFor(() => expect(api.updateProfile).toHaveBeenCalledWith(
      profile.id,
      "Personal",
      undefined,
      {
        notes: "Use for personal work",
      },
    ))
  })

  it("checks and displays live 5-hour, weekly, and reset-credit limits", async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Check limits" }))
    await waitFor(() => expect(api.checkProfileLimits).toHaveBeenCalledWith(profile.id))
    const row = screen.getByTestId(`profile-${profile.id}`)
    expect(row).toHaveTextContent("76% left")
    expect(row).toHaveTextContent("43% left")
    expect(row).toHaveTextContent("Reset credits")
    expect(row).toHaveTextContent("2")
    expect(screen.getByRole("progressbar", { name: "5-hour usage remaining" })).toHaveAttribute("aria-valuenow", "76")
  })

  it("opens reset-credit expiry details returned by Codex", async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Check limits" }))
    await user.click(await screen.findByRole("button", { name: `Show reset-credit expiry for ${profile.name}` }))
    const dialog = screen.getByRole("dialog", { name: "Reset credits" })
    expect(dialog).toHaveTextContent("Usage reset")
    expect(dialog).toHaveTextContent(/Expires/)
  })

  it("explains when Codex returns a reset-credit count without expiry rows", async () => {
    api.checkProfileLimits.mockResolvedValue({ fiveHour: null, weekly: null, monthly: null, resetCreditsAvailable: 2, resetCredits: null, checkedAt: "2026-09-05T04:30:00Z" })
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Check limits" }))
    await user.click(await screen.findByRole("button", { name: `Show reset-credit expiry for ${profile.name}` }))
    expect(screen.getByRole("dialog", { name: "Reset credits" })).toHaveTextContent("did not provide expiry details")
  })

  it("shows per-profile storage and confirms safe cache cleanup", async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: /used by Multi Codex.*View breakdown/ }))
    const storage = await screen.findByRole("dialog", { name: "Profile storage" })
    expect(storage).toHaveTextContent("Personal")
    expect(storage).toHaveTextContent("Safe to clear")
    await user.click(within(storage).getByRole("button", { name: /Clear 244 MB/ }))
    const confirmation = screen.getByRole("alertdialog", { name: /Clear 244 MB from Personal/ })
    expect(confirmation).toHaveTextContent("Credentials, conversations, settings, skills, plugins, and installed extensions stay intact")
    await user.click(within(confirmation).getByRole("button", { name: "Clear cache" }))
    await waitFor(() => expect(api.clearProfileCache).toHaveBeenCalledWith(profile.id))
  })

  it("disables cache cleanup while a profile is running", async () => {
    api.getStorageUsage.mockResolvedValue({
      bytes: 1_000,
      reclaimableBytes: 500,
      otherBytes: 0,
      profiles: [{ id: profile.id, name: profile.name, bytes: 1_000, reclaimableBytes: 500, running: true }],
    })
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: /used by Multi Codex.*View breakdown/ }))
    expect(await screen.findByRole("button", { name: "In use" })).toBeDisabled()
  })

  it("displays the monthly window returned for a free account", async () => {
    api.listProfiles.mockResolvedValue([{ ...profile, accountTier: "Free" }])
    api.checkProfileLimits.mockResolvedValue({
      fiveHour: null,
      weekly: null,
      monthly: { remainingPercent: 37, resetsAt: 1792841898 },
      resetCreditsAvailable: 0,
      checkedAt: "2026-09-27T17:00:00Z",
    })
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Check limits" }))
    const row = await screen.findByLabelText(`Live limits for ${profile.name}`)
    expect(row).toHaveTextContent("Monthly")
    expect(row).toHaveTextContent("37% left")
    expect(row).not.toHaveTextContent("Unavailable")
  })

  it("refreshes all ChatGPT accounts while keeping unavailable limits valid", async () => {
    const second = { ...profile, id: "second", name: "Second", accountTier: "Plus" }
    api.listProfiles.mockResolvedValue([{ ...profile, accountTier: "Free" }, second])
    api.checkProfileLimits
      .mockResolvedValueOnce({ fiveHour: null, weekly: null, monthly: null, resetCreditsAvailable: 0, checkedAt: "2026-09-05T04:30:00Z" })
      .mockResolvedValueOnce({ fiveHour: { remainingPercent: 50, resetsAt: null }, weekly: null, monthly: null, resetCreditsAvailable: null, checkedAt: "2026-09-05T04:30:00Z" })
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Refresh all" }))
    await waitFor(() => expect(api.checkProfileLimits).toHaveBeenCalledWith(profile.id))
    expect(api.checkProfileLimits).toHaveBeenCalledWith(second.id)
    expect(screen.getByTestId(`profile-${profile.id}`)).toHaveTextContent("Free account limits not reported")
    expect(screen.getByTestId(`profile-${profile.id}`)).toHaveTextContent("Free")
    expect(screen.getByTestId(`profile-${second.id}`)).toHaveTextContent("Plus")
  })

  it("does not render a plan badge when the plan is unavailable", async () => {
    render(<App />)
    const row = await screen.findByTestId(`profile-${profile.id}`)
    expect(row.querySelector(".account-tier")).toBeNull()
  })

  it("shows unavailable when the service omits optional limit values", async () => {
    api.checkProfileLimits.mockResolvedValue({
      fiveHour: null,
      weekly: null,
      monthly: null,
      resetCreditsAvailable: null,
      checkedAt: "2026-09-05T04:30:00Z",
    })
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Check limits" }))
    const row = await screen.findByLabelText(`Live limits for ${profile.name}`)
    expect(row.textContent?.match(/Unavailable/g)).toHaveLength(3)
  })

  it("keeps limit errors scoped to the selected profile", async () => {
    api.checkProfileLimits.mockRejectedValue(new Error("Codex limits check timed out"))
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Check limits" }))
    expect(await screen.findByText("Codex limits check timed out")).toBeInTheDocument()
  })

  it("prevents duplicate checks while allowing a later refresh", async () => {
    let resolveCheck: ((value: unknown) => void) | undefined
    api.checkProfileLimits.mockReturnValueOnce(new Promise((resolve) => { resolveCheck = resolve }))
    const user = userEvent.setup()
    render(<App />)
    const button = await screen.findByRole("button", { name: "Check limits" })
    await user.click(button)
    expect(screen.getByRole("button", { name: "Checking" })).toBeDisabled()
    expect(api.checkProfileLimits).toHaveBeenCalledTimes(1)
    resolveCheck?.({
      fiveHour: null,
      weekly: null,
      monthly: null,
      resetCreditsAvailable: 0,
      checkedAt: "2026-09-05T04:30:00Z",
    })
    await user.click(await screen.findByRole("button", { name: "Refresh" }))
    expect(api.checkProfileLimits).toHaveBeenCalledTimes(2)
  })

  it("marks live limits unavailable for API-key profiles", async () => {
    api.listProfiles.mockResolvedValue([{ ...profile, authMode: "API key" }])
    render(<App />)
    expect(await screen.findByRole("button", { name: "Unavailable" })).toBeDisabled()
    expect(api.checkProfileLimits).not.toHaveBeenCalled()
  })

  it("requires explicit confirmation before deletion", async () => {
    const user = userEvent.setup()
    render(<App />)
    await screen.findByRole("heading", { name: "Personal" })
    await user.click(screen.getByRole("button", { name: "Delete Personal" }))
    expect(screen.getByRole("alertdialog")).toHaveTextContent("Delete Personal?")
    expect(api.deleteProfile).not.toHaveBeenCalled()
    const cancel = screen.getByRole("button", { name: "Cancel" })
    await waitFor(() => expect(cancel).toHaveFocus())
    await user.tab({ shift: true })
    expect(screen.getByRole("button", { name: "Delete account" })).toHaveFocus()
    await user.tab()
    expect(cancel).toHaveFocus()
    await user.click(screen.getByRole("button", { name: "Delete account" }))
    await waitFor(() => expect(api.deleteProfile).toHaveBeenCalledWith(profile.id))
  })

  it("toggles and persists the Portfolio-style light and dark themes", async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole("button", { name: "Theme: light. Use dark theme" }))
    expect(document.documentElement).toHaveClass("dark")
    expect(localStorage.getItem("multi-codex-theme")).toBe("dark")
    await user.keyboard("d")
    expect(document.documentElement).not.toHaveClass("dark")
    expect(localStorage.getItem("multi-codex-theme")).toBe("light")
  })

  it("does not use the D theme shortcut while editing a field", async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(await screen.findByRole("button", { name: "Add account" }))
    const input = screen.getByLabelText("Profile name")
    await user.click(input)
    await user.keyboard("d")
    expect(input).toHaveValue("d")
    expect(document.documentElement).not.toHaveClass("dark")
  })

  it("renders a recoverable loading error", async () => {
    api.listProfiles.mockRejectedValueOnce(new Error("keyring unavailable"))
    const user = userEvent.setup()
    render(<App />)
    expect(await screen.findByRole("heading", { name: "Could not load accounts" })).toBeInTheDocument()
    expect(screen.getByRole("alert").textContent).toContain("keyring unavailable")
    await user.click(screen.getByRole("button", { name: "Try again" }))
    expect(await screen.findByRole("heading", { name: "Personal" })).toBeInTheDocument()
  })

  it("offers desktop integration on the first portable AppImage launch", async () => {
    api.getDesktopIntegrationStatus.mockResolvedValue({
      available: true,
      installed: false,
      desktopShortcut: false,
      version: "0.2.0",
      source: "appimage",
    })
    const user = userEvent.setup()
    render(<App />)
    const dialog = await screen.findByRole("dialog", { name: "Add Multi Codex to your apps?" })
    expect(within(dialog).getByRole("checkbox", { name: /Create Desktop shortcut/ })).toBeChecked()
    await user.click(within(dialog).getByRole("button", { name: "Not now" }))
    expect(localStorage.getItem("multi-codex-desktop-dismissed")).toBe("0.2.0")
    expect(screen.getByRole("button", { name: "Install desktop integration" })).toBeEnabled()
  })

  it("installs desktop integration with the selected shortcut preference", async () => {
    api.getDesktopIntegrationStatus.mockResolvedValue({
      available: true,
      installed: false,
      desktopShortcut: false,
      version: "0.2.0",
      source: "appimage",
    })
    api.installDesktopIntegration.mockResolvedValue({
      available: true,
      installed: true,
      desktopShortcut: false,
      version: "0.2.0",
      source: "appimage",
    })
    const user = userEvent.setup()
    render(<App />)
    const dialog = await screen.findByRole("dialog", { name: "Add Multi Codex to your apps?" })
    await user.click(within(dialog).getByRole("checkbox", { name: /Create Desktop shortcut/ }))
    await user.click(within(dialog).getByRole("button", { name: "Install" }))
    await waitFor(() => expect(api.installDesktopIntegration).toHaveBeenCalledWith(false))
    expect(screen.queryByRole("dialog", { name: "Add Multi Codex to your apps?" })).not.toBeInTheDocument()
  })

  it("keeps the integration dialog open when installation fails", async () => {
    api.getDesktopIntegrationStatus.mockResolvedValue({
      available: true,
      installed: false,
      desktopShortcut: false,
      version: "0.2.0",
      source: "appimage",
    })
    api.installDesktopIntegration.mockRejectedValue(new Error("desktop directory is read-only"))
    const user = userEvent.setup()
    render(<App />)
    const dialog = await screen.findByRole("dialog", { name: "Add Multi Codex to your apps?" })
    await user.click(within(dialog).getByRole("button", { name: "Install" }))
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("desktop directory is read-only")
  })

  it("reopens desktop integration after dismissing it", async () => {
    api.getDesktopIntegrationStatus.mockResolvedValue({
      available: true,
      installed: false,
      desktopShortcut: false,
      version: "0.2.0",
      source: "appimage",
    })
    const user = userEvent.setup()
    render(<App />)
    await user.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Not now" }))
    await user.click(screen.getByRole("button", { name: "Install desktop integration" }))
    expect(screen.getByRole("dialog", { name: "Add Multi Codex to your apps?" })).toBeInTheDocument()
  })
})
