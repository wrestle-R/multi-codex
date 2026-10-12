import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, expect, it, vi } from "vitest"
import { SharedStorageDialog } from "./shared-storage-dialog"
import type { MigrationPreview, ProfileStorageUsage } from "../lib/types"

const api = vi.hoisted(() => ({ previewProfileMigration: vi.fn(), migrateProfileSharedStorage: vi.fn(), rollbackProfileSharedStorage: vi.fn(), restoreProfileBackup: vi.fn(), previewLegacyExtensions: vi.fn(), clearLegacyExtensions: vi.fn() }))
vi.mock("../lib/desktop-api", () => api)
const profile: ProfileStorageUsage = { id: "fixture", name: "Personal", bytes: 100, reclaimableBytes: 0, running: false, layout: "legacy" }
const preview: MigrationPreview = { profileId: "fixture", sourceBytes: 100, entries: 2, requiredBackupBytes: 1000, availableBackupBytes: 5000, requiredAppBytes: 2000, availableAppBytes: 9000, externalLinks: [], running: false, snapshot: null }
beforeEach(() => {
  api.previewProfileMigration.mockReset().mockResolvedValue(preview)
  api.migrateProfileSharedStorage.mockReset().mockResolvedValue({ phase: "committed" })
  api.rollbackProfileSharedStorage.mockReset().mockResolvedValue({ phase: "rolledBack" })
})
afterEach(cleanup)

it.each([
  { running: true }, { externalLinks: ["uncovered"] }, { availableAppBytes: 1 }, { availableBackupBytes: 1 }, { requiredAppBytes: undefined },
])("defers migration when preservation preflight is incomplete: %j", async status => {
  api.previewProfileMigration.mockResolvedValue({ ...preview, ...status })
  render(<SharedStorageDialog profile={profile} onClose={vi.fn()} onCompleted={vi.fn()} />)
  await waitFor(() => expect(api.previewProfileMigration).toHaveBeenCalled())
  await waitFor(() => expect(screen.queryByText("Checking profile sessions and migration space…")).not.toBeInTheDocument())
  expect(screen.getByRole("button", { name: "Back up and enable sharing" })).toBeDisabled()
  expect(api.migrateProfileSharedStorage).not.toHaveBeenCalled()
})
it("reports sharing only after native commit and prevents duplicate migration", async () => {
  let complete!: (result: unknown) => void
  api.migrateProfileSharedStorage.mockImplementation(() => new Promise(resolve => { complete = resolve }))
  const completed = vi.fn()
  render(<SharedStorageDialog profile={profile} onClose={vi.fn()} onCompleted={completed} />)
  await waitFor(() => expect(screen.getByRole("button", { name: "Back up and enable sharing" })).toBeEnabled())
  fireEvent.click(screen.getByRole("button", { name: "Back up and enable sharing" }))
  expect(screen.getByRole("button", { name: "Working…" })).toBeDisabled()
  expect(screen.getByRole("button", { name: "Close" })).toBeDisabled()
  expect(completed).not.toHaveBeenCalled()
  complete({ phase: "committed" })
  await waitFor(() => expect(completed).toHaveBeenCalledWith(expect.stringContaining("verified backup")))
  expect(api.migrateProfileSharedStorage).toHaveBeenCalledTimes(1)
})
it("recovers interrupted settings without requiring another package/backup allocation", async () => {
  api.previewProfileMigration.mockResolvedValue({ ...preview, availableAppBytes: 0, availableBackupBytes: 0, externalLinks: ["retained-generation"] })
  const completed = vi.fn()
  render(<SharedStorageDialog profile={{ ...profile, layout: "needsRecovery" }} onClose={vi.fn()} onCompleted={completed} />)
  await waitFor(() => expect(screen.getByRole("button", { name: "Return to original layout" })).toBeEnabled())
  fireEvent.click(screen.getByRole("button", { name: "Return to original layout" }))
  await waitFor(() => expect(completed).toHaveBeenCalledWith(expect.stringContaining("Shared chats and backups are retained")))
  expect(api.rollbackProfileSharedStorage).toHaveBeenCalledWith(profile.id)
  expect(api.migrateProfileSharedStorage).not.toHaveBeenCalled()
})
it("requires a fresh preview after a late client or import failure", async () => {
  api.migrateProfileSharedStorage.mockRejectedValue(new Error("A profile client started; the original layout is retained"))
  const completed = vi.fn()
  render(<SharedStorageDialog profile={profile} onClose={vi.fn()} onCompleted={completed} />)
  await waitFor(() => expect(screen.getByRole("button", { name: "Back up and enable sharing" })).toBeEnabled())
  fireEvent.click(screen.getByRole("button", { name: "Back up and enable sharing" }))
  expect(await screen.findByRole("alert")).toHaveTextContent("original layout is retained")
  expect(completed).not.toHaveBeenCalled()
  expect(screen.getByRole("button", { name: "Back up and enable sharing" })).toBeDisabled()
})

it("requires an explicit second action to activate a verified backup restore", async () => {
  api.previewProfileMigration.mockResolvedValue({ ...preview, snapshot: { state: "verified" } })
  api.restoreProfileBackup.mockResolvedValue({ phase: "complete" })
  const completed = vi.fn()
  render(<SharedStorageDialog profile={profile} onClose={vi.fn()} onCompleted={completed} />)
  fireEvent.click(await screen.findByRole("button", { name: "Restore backup" }))
  expect(api.restoreProfileBackup).not.toHaveBeenCalled()
  expect(screen.getByRole("button", { name: "Back up and enable sharing" })).toBeDisabled()
  fireEvent.click(screen.getByRole("button", { name: "Confirm backup restore" }))
  await waitFor(() => expect(completed).toHaveBeenCalledWith(expect.stringContaining("current login")))
  expect(api.restoreProfileBackup).toHaveBeenCalledWith(profile.id)
})

it("shows original-extension bytes before clearing and preserves the recovery promise", async () => {
  api.previewLegacyExtensions.mockResolvedValue({ token: "exact-preview", bytes: 4096 })
  api.clearLegacyExtensions.mockResolvedValue(4096)
  const completed = vi.fn()
  render(<SharedStorageDialog profile={{ ...profile, layout: "shared" }} onClose={vi.fn()} onCompleted={completed} />)
  await waitFor(() => expect(screen.getByRole("button", { name: "Preview original extension cleanup" })).toBeEnabled())
  fireEvent.click(screen.getByRole("button", { name: "Preview original extension cleanup" }))
  const confirm = await screen.findByRole("button", { name: "Confirm original extension cleanup" })
  expect(api.clearLegacyExtensions).not.toHaveBeenCalled()
  fireEvent.click(confirm)
  await waitFor(() => expect(api.clearLegacyExtensions).toHaveBeenCalledWith(profile.id, "exact-preview"))
  expect(completed).toHaveBeenCalledWith(expect.stringContaining("verified recovery backup"))
})
