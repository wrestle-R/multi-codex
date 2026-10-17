import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, expect, it, vi } from "vitest"
import { SharedChatCleanupDialog } from "./shared-chat-cleanup-dialog"
const api = vi.hoisted(() => ({ previewSharedChats: vi.fn(), clearSharedChats: vi.fn() }))
vi.mock("../lib/desktop-api", () => api)
afterEach(() => { cleanup(); vi.resetAllMocks() })
it("uses the exact global preview and disables retries until a changed library is checked again", async () => {
  api.previewSharedChats.mockResolvedValue({ token: "global-preview", count: 2, protectedCount: 1, transcriptBytes: 0 })
  api.clearSharedChats.mockRejectedValue(new Error("Shared chats changed. Check the preview again"))
  const completed = vi.fn()
  render(<SharedChatCleanupDialog scope="all" onClose={vi.fn()} onCompleted={completed} />)
  expect(screen.getByText(/every account/)).toBeVisible()
  await waitFor(() => expect(screen.getByRole("button", { name: "Clear 2 shared chats" })).toBeEnabled())
  fireEvent.click(screen.getByRole("button", { name: "Clear 2 shared chats" }))
  expect(await screen.findByRole("alert")).toHaveTextContent("Check the preview again")
  expect(api.clearSharedChats).toHaveBeenCalledWith("global-preview")
  expect(screen.getByRole("button", { name: "Clear 0 shared chats" })).toBeDisabled()
  expect(completed).not.toHaveBeenCalled()
})
