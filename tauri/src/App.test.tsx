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
  launchProfile: vi.fn(),
  deleteProfile: vi.fn(),
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
  api.launchProfile.mockReset().mockResolvedValue(undefined)
  api.deleteProfile.mockReset().mockResolvedValue(undefined)
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
    await waitFor(() => expect(api.importCurrentProfile).toHaveBeenCalledWith("Current"))
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
})
