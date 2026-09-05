import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import App from "./App"
import type { Profile } from "./lib/types"

const api = vi.hoisted(() => ({
  listProfiles: vi.fn(),
  addProfile: vi.fn(),
  importCurrentProfile: vi.fn(),
  updateProfile: vi.fn(),
  checkProfileLimits: vi.fn(),
  launchProfile: vi.fn(),
  deleteProfile: vi.fn(),
  getDesktopIntegrationStatus: vi.fn(),
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

beforeEach(() => {
  localStorage.clear()
  api.listProfiles.mockReset().mockResolvedValue([profile])
  api.addProfile.mockReset().mockResolvedValue(profile)
  api.importCurrentProfile.mockReset().mockResolvedValue(profile)
  api.updateProfile.mockReset().mockResolvedValue(profile)
  api.checkProfileLimits.mockReset().mockResolvedValue({
    fiveHour: { remainingPercent: 76, resetsAt: 1788597000 },
    weekly: { remainingPercent: 43, resetsAt: 1788998400 },
    resetCreditsAvailable: 2,
    checkedAt: "2026-09-05T04:30:00Z",
  })
  api.launchProfile.mockReset().mockResolvedValue(undefined)
  api.deleteProfile.mockReset().mockResolvedValue(undefined)
  api.getDesktopIntegrationStatus.mockReset().mockResolvedValue({
    available: false,
    installed: true,
    desktopShortcut: false,
    version: "0.2.0",
    source: "package",
  })
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
  it("loads profiles and exposes profile actions", async () => {
    render(<App />)
    expect(await screen.findByRole("heading", { name: "Personal" })).toBeInTheDocument()
    expect(screen.getByRole("button", { name: "Edit Personal" })).toBeEnabled()
    expect(screen.getByRole("button", { name: "Delete Personal" })).toBeEnabled()
    expect(screen.getByRole("button", { name: "Launch" })).toBeEnabled()
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

  it("shows unavailable when the service omits optional limit values", async () => {
    api.checkProfileLimits.mockResolvedValue({
      fiveHour: null,
      weekly: null,
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

  it("cycles through system, light, and dark themes", async () => {
    const user = userEvent.setup()
    render(<App />)
    const system = screen.getByRole("button", { name: "Theme: system. Use light theme" })
    await user.click(system)
    expect(screen.getByRole("button", { name: "Theme: light. Use dark theme" })).toBeInTheDocument()
    await user.click(screen.getByRole("button", { name: "Theme: light. Use dark theme" }))
    expect(document.documentElement).toHaveClass("dark")
    expect(localStorage.getItem("multi-codex-theme")).toBe("dark")
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
