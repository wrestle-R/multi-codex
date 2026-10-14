import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, expect, it, vi } from "vitest"
import Hud from "./Hud"
import { defaultHudSettings, sessionSlots, type HudSession, type HudView } from "./lib/hud"

const api = vi.hoisted(() => ({ call: vi.fn() }))
vi.mock("./lib/hud", async importOriginal => ({ ...await importOriginal<typeof import("./lib/hud")>(), nativeHud: false, hudCall: api.call }))
vi.mock("./components/account-avatar", () => ({ AccountAvatar: () => <span data-testid="plush-avatar" /> }))
const session = (id: string, needsAttention = false): HudSession => ({ id, title: `Session ${id}`, cwd: "/tmp", status: "idle", attention: needsAttention ? "mayNeedYou" : null, needsAttention, eventKey: `account/${id}/1`, epoch: 1, tokens: null })
let view: HudView
beforeEach(() => {
  sessionStorage.clear(); localStorage.clear()
  view = { settings: defaultHudSettings, accounts: [{ id: "account", name: "Fixture", avatar: { kind: "ghost", color: "#c6b9e8", seed: 1 }, limits: { primary: { usedPercent: 20, resetsAt: null, windowDurationMins: 300 }, secondary: { usedPercent: 50, resetsAt: null, windowDurationMins: 10080 } }, sessions: [session("one")] }] }
  api.call.mockReset().mockImplementation((method: string) => Promise.resolve(method === "hud_snapshot" ? view : undefined))
})
afterEach(cleanup)
it("hovers for quota and clicks to open the exact saved account/chat", async () => {
  render(<Hud />)
  const avatar = await screen.findByRole("button", { name: "Fixture: Open" })
  fireEvent.mouseEnter(avatar.parentElement!)
  expect(await screen.findByText("80% left")).toBeVisible()
  expect(api.call).not.toHaveBeenCalledWith("hud_open_session", expect.anything())
  fireEvent.click(avatar)
  await waitFor(() => expect(api.call).toHaveBeenCalledWith("hud_open_session", { accountId: "account", threadId: "one", eventKey: null }))
})
it("subdivides equal account slots, prioritizes attention and keeps all overflow chats accessible", async () => {
  view.accounts[0].sessions = [session("one"), session("two"), session("three"), session("four"), session("five", true), session("six")]
  expect(sessionSlots(view.accounts[0].sessions).map(slot => "overflow" in slot ? slot.overflow : slot.id)).toEqual(["five", "four", "one", 3])
  render(<Hud />)
  const overflow = await screen.findByRole("button", { name: "3 more Fixture chats" })
  fireEvent.click(overflow)
  expect(await screen.findByRole("button", { name: /Session six/ })).toBeVisible()
  fireEvent.click(screen.getByRole("button", { name: /Session fiveMay need you/ }))
  await waitFor(() => expect(api.call).toHaveBeenCalledWith("hud_open_session", { accountId: "account", threadId: "five", eventKey: "account/five/1" }))
})
it("shows an icon-only reminder with no chat title or reply controls", async () => {
  view.accounts[0].sessions = [session("private-title", true)]
  render(<Hud />)
  const popup = await screen.findByRole("button", { name: "Open chat needing attention" })
  expect(popup).not.toHaveTextContent("private-title")
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument()
  fireEvent.click(popup)
  await waitFor(() => expect(api.call).toHaveBeenCalledWith("hud_open_session", expect.objectContaining({ eventKey: "account/private-title/1" })))
})

it("disabling Orbit suppresses Orbiters, popups and sound while leaving chats untouched", async () => {
  const sound = vi.fn()
  vi.stubGlobal("AudioContext", sound)
  view.settings = { ...defaultHudSettings, enabled: false, sound: true }
  view.accounts[0].sessions = [session("waiting", true)]
  const { container } = render(<Hud />)
  await waitFor(() => expect(container.querySelector(".hud-shell")).toBeNull())
  expect(sound).not.toHaveBeenCalled()
  expect(api.call).not.toHaveBeenCalledWith("hud_open_session", expect.anything())
  vi.unstubAllGlobals()
})
