import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, expect, it, vi } from "vitest"
import { MigrationBackupDialog } from "./migration-backup-dialog"
import type { MigrationPreview, MigrationSnapshot, ProfileStorageUsage } from "../lib/types"

const api = vi.hoisted(() => ({ previewProfileMigration: vi.fn(), prepareProfileMigration: vi.fn() }))
vi.mock("../lib/desktop-api", () => api)
const profile: ProfileStorageUsage = { id: "fixture", name: "Personal", bytes: 100, reclaimableBytes: 0, running: false }
const preview: MigrationPreview = { profileId: profile.id, sourceBytes: 100, entries: 2, requiredBackupBytes: 1000, availableBackupBytes: 5000, externalLinks: [], running: false, snapshot: null }
const snapshot: MigrationSnapshot = { schema: 1, profileId: profile.id, snapshotId: "fixture-copy", state: "verified", sourceDigest: "fixture", metadataDigest: "fixture", sourceBytes: 100, entries: 2, createdAt: "2026-10-12T00:00:00Z", verifiedAt: "2026-10-12T00:00:01Z" }

beforeEach(() => {
  api.previewProfileMigration.mockReset().mockResolvedValue(preview)
  api.prepareProfileMigration.mockReset().mockResolvedValue(snapshot)
})
afterEach(cleanup)

it.each([
  { running: true, externalLinks: [], availableBackupBytes: 5000 },
  { running: false, externalLinks: ["external"], availableBackupBytes: 5000 },
  { running: false, externalLinks: [], availableBackupBytes: 100 },
])("blocks unsafe backup readiness: %j", async status => {
  api.previewProfileMigration.mockResolvedValue({ ...preview, ...status })
  render(<MigrationBackupDialog profile={profile} onClose={vi.fn()} onPrepared={vi.fn()} />)
  await screen.findByText("100 B of profile files")
  expect(screen.getByRole("button", { name: "Create verified backup" })).toBeDisabled()
  expect(api.prepareProfileMigration).not.toHaveBeenCalled()
})

it("reports success only after native verification and prevents duplicate requests while busy", async () => {
  let complete!: (snapshot: MigrationSnapshot) => void
  api.prepareProfileMigration.mockImplementation(() => new Promise(resolve => { complete = resolve }))
  const prepared = vi.fn()
  render(<MigrationBackupDialog profile={profile} onClose={vi.fn()} onPrepared={prepared} />)
  const button = await screen.findByRole("button", { name: "Create verified backup" })
  await waitFor(() => expect(button).toBeEnabled())
  fireEvent.click(button)
  expect(prepared).not.toHaveBeenCalled()
  expect(screen.getByRole("button", { name: "Verifying backup…" })).toBeDisabled()
  expect(screen.getByRole("button", { name: "Close" })).toBeDisabled()
  complete(snapshot)
  await waitFor(() => expect(prepared).toHaveBeenCalledWith(snapshot))
  expect(api.prepareProfileMigration).toHaveBeenCalledTimes(1)
})

it("requires a fresh preview after a late native activity failure", async () => {
  api.prepareProfileMigration.mockRejectedValue(new Error("A profile client started during backup"))
  const prepared = vi.fn()
  render(<MigrationBackupDialog profile={profile} onClose={vi.fn()} onPrepared={prepared} />)
  await waitFor(() => expect(screen.getByRole("button", { name: "Create verified backup" })).toBeEnabled())
  fireEvent.click(screen.getByRole("button", { name: "Create verified backup" }))
  expect(await screen.findByRole("alert")).toHaveTextContent("client started")
  expect(screen.getByRole("button", { name: "Create verified backup" })).toBeDisabled()
  expect(prepared).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole("button", { name: "Check again" }))
  await waitFor(() => expect(api.previewProfileMigration).toHaveBeenCalledTimes(2))
})
