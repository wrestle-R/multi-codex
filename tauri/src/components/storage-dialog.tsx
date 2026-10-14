import { Delete02Icon, Refresh01Icon } from "@hugeicons/core-free-icons"
import { HugeiconsIcon } from "@hugeicons/react"
import { useId } from "react"
import type { ChatScope, Profile, ProfileStorageUsage, StorageUsage } from "../lib/types"
import { formatStorage } from "../lib/formatters"
import { useDialogFocus } from "./use-dialog-focus"

interface StorageDialogProps {
  usage: StorageUsage | null
  loading: boolean
  error: string | null
  onClose: () => void
  onRefresh: () => void
  onClear: (profile: ProfileStorageUsage) => void
  onDelete: (profile: Profile) => void
  onBackup: (profile: ProfileStorageUsage) => void
  onMigrate: (profile: ProfileStorageUsage) => void
  onManageSharing: () => void
  onClearSharedChats: (scope: ChatScope) => void
  onClearChats: (profile: ProfileStorageUsage, scope: ChatScope) => void
  notice?: string | null
  profiles: Profile[]
}

export function StorageDialog({ usage, loading, error, onClose, onRefresh, onClear, onDelete, onClearChats, onBackup, onMigrate, onClearSharedChats, onManageSharing, notice, profiles }: StorageDialogProps) {
  const titleId = useId()
  const dialogRef = useDialogFocus(onClose, false)

  return (
    <div className="dialog-layer" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <section ref={dialogRef} className="dialog storage-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <div className="dialog-header">
          <div>
            <span className="eyebrow">Local storage</span>
            <h2 id={titleId}>Profile storage</h2>
          </div>
          <button className="dialog-close" type="button" aria-label="Close" onClick={onClose}>×</button>
        </div>
        {usage ? (
          <div className="storage-summary">
            <div><strong>{formatStorage(usage.bytes)}</strong><span>On disk</span></div>
            <div><strong>{formatStorage(usage.reclaimableBytes)}</strong><span>Safe to clear</span></div>
          </div>
        ) : null}
        <div className="storage-toolbar">
          <p>Clear unused cache. Your logins, chats and installed tools stay intact.</p>
          <button className="icon-button" type="button" aria-label="Refresh storage" disabled={loading} onClick={onRefresh}>
            <HugeiconsIcon icon={Refresh01Icon} size={18} strokeWidth={1.8} />
          </button>
        </div>
        {error ? <div className="form-error" role="alert">{error}</div> : null}
        {notice ? <p className="storage-notice" role="status">{notice}</p> : null}
        {loading && !usage ? <div className="storage-loading">Calculating profile sizes…</div> : null}
        {usage ? (
          <div className="storage-list">
            {usage.profiles.map((item) => {
              const profile = profiles.find((candidate) => candidate.id === item.id)
              return (
                <article className="storage-row" key={item.id}>
                  <div className="storage-row-copy">
                    <strong>{item.name}</strong>
                    <span>{formatStorage(item.bytes)} total · {formatStorage(item.reclaimableBytes)} cache</span>
                  </div>
                  <div className="storage-bar" aria-hidden="true"><span style={{ width: `${usage.bytes ? Math.max(2, item.bytes / usage.bytes * 100) : 0}%` }} /></div>
                  <div className="storage-row-actions">
                    <button className="button secondary cache-button" type="button" disabled={loading || item.running || item.reclaimableBytes === 0} title={item.running ? "Close this profile's VS Code, Codex Desktop, and CLI sessions first" : undefined} onClick={() => onClear(item)}>
                      {item.running ? "In use" : item.reclaimableBytes === 0 ? "Clean" : `Clear ${formatStorage(item.reclaimableBytes)}`}
                    </button>

                  </div>
                  <details className="storage-row-details">
                    <summary>Details</summary>
                    {item.breakdown ? <dl className="storage-breakdown">
                      <div><dt>VS Code data</dt><dd>{formatStorage(item.breakdown.vscodeBytes)}</dd></div>
                      <div><dt>Installed extensions</dt><dd>{formatStorage(item.breakdown.extensionsBytes)}</dd></div>
                      <div><dt>Codex data, including chats</dt><dd>{formatStorage(item.breakdown.codexBytes)}</dd></div>
                      <div><dt>Codex Desktop data</dt><dd>{formatStorage(item.breakdown.desktopBytes)}</dd></div>
                      {item.privateCatalogBytes ? <div><dt>Private tools and registry</dt><dd>{formatStorage(item.privateCatalogBytes)}</dd></div> : null}
                      {item.privateRuntimeBytes ? <div><dt>Working copies</dt><dd>{formatStorage(item.privateRuntimeBytes)}</dd></div> : null}
                    </dl> : null}
                    <p className="storage-chat-help">{item.layout === "shared" ? "Profile cleanup affects its retained private histories. Shared conversations remain in the shared library." : "Chat cleanup removes local conversations and their history records. Your account and installed tools are kept."}</p>
                    <div className="storage-chat-actions">
                      <button className="button secondary" type="button" disabled={loading} aria-label={`Back up ${item.name}`} onClick={() => onBackup(item)}>Back up profile</button>
                      <button className="button secondary" type="button" disabled={loading || item.running} onClick={() => onMigrate(item)}>{item.layout === "shared" ? "Shared storage and recovery" : item.layout === "needsRecovery" || item.layout === "needsRestore" ? "Recover interrupted migration" : "Set up shared storage"}</button>
                      <button className="button secondary" type="button" disabled={item.running || loading} aria-label={`Clear chats older than 30 days for ${item.name}`} onClick={() => onClearChats(item, "olderThan30Days")}>Clear chats older than 30 days</button>
                      <button className="button secondary danger-button" type="button" disabled={item.running || loading} aria-label={`Clear all chats for ${item.name}`} onClick={() => onClearChats(item, "all")}>Clear all chats</button>
                    </div>
                    <button className="icon-button danger-button" type="button" disabled={!profile} aria-label={`Delete ${item.name} from storage`} onClick={() => profile && onDelete(profile)}>
                      <HugeiconsIcon icon={Delete02Icon} size={18} strokeWidth={1.8} />
                    </button>
                    {item.running ? <p className="storage-chat-help">Close this profile’s VS Code, Codex Desktop, and CLI sessions to clear chats.</p> : null}
                  </details>
                </article>
              )
            })}
            <details className="storage-advanced">
              <summary>Shared files and advanced options</summary>
              <dl className="storage-breakdown">
                <div><dt>Shared extensions</dt><dd>{formatStorage(usage.sharedExtensionsBytes ?? 0)}</dd></div>
                <div><dt>Shared chats</dt><dd>{formatStorage(usage.sharedChatsBytes ?? 0)}</dd></div>
                <div><dt>Migration recovery</dt><dd>{formatStorage(usage.migrationBytes ?? 0)}</dd></div>
                <div><dt>App metadata</dt><dd>{formatStorage(usage.otherBytes)}</dd></div>
                <div><dt>Backups, outside app total</dt><dd>{formatStorage(usage.backupBytes ?? 0)}</dd></div>
              </dl>
              <p className="storage-chat-help">Shared files are counted once. Account rows show private disk space. Active accounts must close before cache cleanup.</p>
              <div className="storage-chat-actions">
                <button className="button secondary" disabled={loading} onClick={onManageSharing}>Manage sharing</button>
                <button className="button secondary" disabled={loading} onClick={() => onClearSharedChats("olderThan30Days")}>Clear shared chats older than 30 days</button>
                <button className="button secondary danger-button" disabled={loading} onClick={() => onClearSharedChats("all")}>Clear all shared chats</button>
              </div>
              <p className="storage-chat-help">Shared chat deletion affects every saved account.</p>
            </details>
          </div>
        ) : null}
      </section>
    </div>
  )
}
