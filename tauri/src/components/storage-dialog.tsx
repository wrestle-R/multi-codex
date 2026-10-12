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
  onClearChats: (profile: ProfileStorageUsage, scope: ChatScope) => void
  notice?: string | null
  profiles: Profile[]
}

export function StorageDialog({ usage, loading, error, onClose, onRefresh, onClear, onDelete, onClearChats, notice, profiles }: StorageDialogProps) {
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
            <div><strong>{formatStorage(usage.bytes)}</strong><span>Total used</span></div>
            <div><strong>{formatStorage(usage.reclaimableBytes)}</strong><span>Safe to clear</span></div>
          </div>
        ) : null}
        <div className="storage-toolbar">
          <p>Cache cleanup keeps credentials, conversations, settings, skills, plugins, and installed extensions.</p>
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
                    <button className="button secondary cache-button" type="button" disabled={item.running || item.reclaimableBytes === 0} title={item.running ? "Close this profile's VS Code, Codex Desktop, and CLI sessions first" : undefined} onClick={() => onClear(item)}>
                      {item.running ? "In use" : item.reclaimableBytes === 0 ? "Clean" : `Clear ${formatStorage(item.reclaimableBytes)}`}
                    </button>
                    <button className="icon-button danger-button" type="button" disabled={!profile} aria-label={`Delete ${item.name} from storage`} onClick={() => profile && onDelete(profile)}>
                      <HugeiconsIcon icon={Delete02Icon} size={18} strokeWidth={1.8} />
                    </button>
                  </div>
                  <details className="storage-row-details">
                    <summary>Data and chats<span>{item.breakdown ? `${formatStorage(item.breakdown.chatBytes)} of chat transcripts` : "View cleanup options"}</span></summary>
                    {item.breakdown ? <dl className="storage-breakdown">
                      <div><dt>VS Code data</dt><dd>{formatStorage(item.breakdown.vscodeBytes)}</dd></div>
                      <div><dt>Installed extensions</dt><dd>{formatStorage(item.breakdown.extensionsBytes)}</dd></div>
                      <div><dt>Codex data, including chats</dt><dd>{formatStorage(item.breakdown.codexBytes)}</dd></div>
                      <div><dt>Codex Desktop data</dt><dd>{formatStorage(item.breakdown.desktopBytes)}</dd></div>
                    </dl> : null}
                    <p className="storage-chat-help">Chat cleanup removes local conversations and their history records. Your account and installed tools are kept.</p>
                    <div className="storage-chat-actions">
                      <button className="button secondary" type="button" disabled={item.running || loading} aria-label={`Clear chats older than 30 days for ${item.name}`} onClick={() => onClearChats(item, "olderThan30Days")}>Clear chats older than 30 days</button>
                      <button className="button secondary danger-button" type="button" disabled={item.running || loading} aria-label={`Clear all chats for ${item.name}`} onClick={() => onClearChats(item, "all")}>Clear all chats</button>
                    </div>
                    {item.running ? <p className="storage-chat-help">Close this profile’s VS Code, Codex Desktop, and CLI sessions to clear chats.</p> : null}
                  </details>
                </article>
              )
            })}
            {usage.otherBytes > 0 ? (
              <article className="storage-row storage-other">
                <div className="storage-row-copy"><strong>App metadata</strong><span>{formatStorage(usage.otherBytes)}</span></div>
              </article>
            ) : null}
          </div>
        ) : null}
      </section>
    </div>
  )
}
