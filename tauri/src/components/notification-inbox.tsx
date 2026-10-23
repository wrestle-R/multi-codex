import { useId } from "react"
import type { AppNotification } from "../lib/notifications"
import { useDialogFocus } from "./use-dialog-focus"
export function NotificationInbox({ items, error, onRead, onClear, onSettings, onClose }: { items: AppNotification[]; error: string; onRead: () => void; onClear: () => void; onSettings: () => void; onClose: () => void }) {
  const title = useId()
  const ref = useDialogFocus(onClose, false)
  return <div className="dialog-layer"><section className="dialog preference-dialog" role="dialog" aria-modal="true" aria-labelledby={title} ref={ref}>
    <header className="dialog-header"><div><p className="eyebrow">Your accounts</p><h2 id={title}>Notifications</h2></div><button type="button" className="icon-button" aria-label="Close notifications" onClick={onClose}>×</button></header>
    <div className="preference-body notification-list">
      {error && <p className="form-error" role="status">{error}</p>}
      {!items.length && <div className="notification-empty"><strong>You’re all caught up.</strong><p>Resets, low usage, expired sign-ins and updates appear here.</p></div>}
      {items.map(item => <article key={item.id} className={item.read ? "notification-item" : "notification-item unread"}><div><strong>{item.title}</strong><time dateTime={new Date(item.createdAt).toISOString()}>{new Date(item.createdAt).toLocaleString()}</time></div><p>{item.body}</p></article>)}
    </div><div className="dialog-actions"><button type="button" className="button secondary" onClick={onSettings}>Notification settings</button><button type="button" className="button secondary" disabled={!items.some(item => !item.read)} onClick={onRead}>Mark all read</button><button type="button" className="button secondary" disabled={!items.length} onClick={onClear}>Clear</button></div>
  </section></div>
}
