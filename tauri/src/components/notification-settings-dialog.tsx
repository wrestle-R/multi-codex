import { useId, useState } from "react"
import { savePreferences, useAppPreferences, type AppPreferences } from "../lib/app-preferences"
import { enableDesktopNotifications, deliverDesktopNotification } from "./use-notifications"
import { useDialogFocus } from "./use-dialog-focus"
import type { Profile } from "../lib/types"

export function NotificationSettingsDialog({ profiles, onClose }: { profiles: Profile[]; onClose: () => void }) {
  const prefs = useAppPreferences()
  const [draft, setDraft] = useState(prefs)
  const [busy, setBusy] = useState(false)
  const [notice, setNotice] = useState("")
  const title = useId()
  const ref = useDialogFocus(onClose, busy)
  const update = <K extends keyof AppPreferences>(key: K, value: AppPreferences[K]) => setDraft(current => ({ ...current, [key]: value }))
  const toggle = (key: "notifications" | "updates" | "bankedResets" | "weeklyResets" | "expiringResets" | "signInExpired", label: string, description: string) => <label className="desktop-picker-setting"><input type="checkbox" checked={draft[key]} onChange={event => update(key, event.target.checked)} /><span className="desktop-setting-copy">{label}<small>{description}</small></span><span className="desktop-switch-track" aria-hidden="true" /></label>
  return <div className="dialog-layer"><section className="dialog preference-dialog notification-settings-dialog" role="dialog" aria-modal="true" aria-labelledby={title} ref={ref}>
    <header className="dialog-header"><div><p className="eyebrow">Settings</p><h2 id={title}>Notifications</h2></div><button type="button" className="icon-button" aria-label="Close notification settings" disabled={busy} onClick={onClose}>×</button></header>
    <form onSubmit={event => { event.preventDefault(); try { savePreferences(draft); onClose() } catch { setNotice("Could not save notification preferences. Check available storage.") } }}>
      <div className="preference-body">
        {toggle("notifications", "Enable notifications", "Account alerts and app updates.")}
        <fieldset disabled={!draft.notifications || busy} className="notification-fields">
          <section className="settings-section"><div className="settings-section-heading"><h3>Alerts</h3></div>
            {toggle("bankedResets", "New banked reset", "A new reset saved for later.")}
            {toggle("weeklyResets", "Weekly usage reset", "Your weekly allowance is restored.")}
            {toggle("signInExpired", "Sign-in expired", "An account needs you to sign in again.")}
            {toggle("updates", "App update available", "A new version is ready.")}
            {toggle("expiringResets", "Expiring-reset reminder", "A saved reset expires within 48 hours.")}
          </section>
          <section className="settings-section"><div className="settings-section-heading"><h3>Low-usage alerts</h3><p>Choose one or more levels.</p></div><div className="threshold-choices">{[40, 30, 20].map(value => <label key={value}><input type="checkbox" checked={draft.thresholds.includes(value)} onChange={event => update("thresholds", event.target.checked ? [...draft.thresholds, value] : draft.thresholds.filter(t => t !== value))} />Below {value}%</label>)}</div><p className="settings-help">One alert per crossed level, for each usage window.</p></section>
          <section className="settings-section"><div className="settings-section-heading"><h3>Accounts</h3><p>Choose which accounts can notify you.</p></div>{profiles.filter(p => p.authMode.toLowerCase() === "chatgpt").map(profile => <label className="notification-account" key={profile.id}><span>{profile.name}</span><input type="checkbox" aria-label={`Notifications for ${profile.name}`} checked={!draft.mutedAccounts.includes(profile.id)} onChange={event => update("mutedAccounts", event.target.checked ? draft.mutedAccounts.filter(id => id !== profile.id) : [...draft.mutedAccounts, profile.id])} /></label>)}</section>
          <section className="settings-section"><div className="settings-section-heading"><h3>Desktop alerts</h3><p>Optional system notifications.</p></div>
            <label className="desktop-picker-setting"><input type="checkbox" checked={draft.desktopNotifications} onChange={event => {
              if (!event.target.checked) { update("desktopNotifications", false); return }
              setBusy(true); setNotice("")
              void enableDesktopNotifications().then(granted => { update("desktopNotifications", granted); if (!granted) setNotice("Desktop permission is unavailable or denied. Allow Multi Codex in your system notification settings. Browser previews keep alerts in the inbox.") }).catch(() => setNotice("Could not enable desktop alerts. Your in-app inbox still works.")).finally(() => setBusy(false))
            }} /><span className="desktop-setting-copy">Show desktop notifications<small>Show alerts while you’re using another app.</small></span><span className="desktop-switch-track" aria-hidden="true" /></label>
            <button type="button" className="button secondary" disabled={!draft.desktopNotifications} onClick={() => { void deliverDesktopNotification({ title: "Multi Codex · Test notification", body: "Desktop account alerts are enabled." }).then(() => setNotice("Test notification requested. System Focus or Do Not Disturb may hide it.")).catch(() => setNotice("Could not send a desktop alert. Check system notification permissions.")) }}>Send test notification</button>
          </section>
        </fieldset>
        <p className="settings-help">Checked every {prefs.refreshMinutes} minutes. Latest 100 alerts saved here.</p>
        {notice && <p className="desktop-notice" role="status">{notice}</p>}
      </div><div className="dialog-actions"><button type="button" className="button secondary" disabled={busy} onClick={onClose}>Cancel</button><button className="button primary" disabled={busy}>Save settings</button></div>
    </form>
  </section></div>
}
