import { render } from "@testing-library/react"
import { expect, it, vi } from "vitest"
import { AccountAvatar } from "./account-avatar"
const draw = vi.hoisted(() => vi.fn())
vi.mock("bot-avatars", () => ({ BotAvatar: (props: object) => { draw(props); return <canvas /> } }))
it("reuses a still avatar when IPC refreshes return the same identity in a new object", () => {
  const identity = { kind: "ghost" as const, color: "#c6b9e8", seed: 42 }
  const view = render(<AccountAvatar id="account" identity={identity} size={56} animate={false} />)
  const initial = draw.mock.calls.length
  view.rerender(<AccountAvatar id="account" identity={{ ...identity }} size={56} animate={false} />)
  expect(draw).toHaveBeenCalledTimes(initial)
  expect(draw.mock.lastCall?.[0]).toEqual(expect.objectContaining({ paused: true, interactive: false }))
  view.rerender(<AccountAvatar id="account" identity={{ ...identity, color: "#60c8df" }} size={56} animate={false} />)
  expect(draw.mock.calls.length).toBeGreaterThan(initial)
})
