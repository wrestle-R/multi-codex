import { useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification"
import { notificationKey, observeAccount, observeSignIn, readNotifications, type AppNotification } from "../lib/notifications"
import type { AppPreferences } from "../lib/app-preferences"
import type { LimitCheckState, Profile } from "../lib/types"

const native = Boolean(window.__TAURI_INTERNALS__) && new URLSearchParams(location.search).get("demo") !== "1"
export async function enableDesktopNotifications(): Promise<boolean> {
  if (!native) return false
  return await isPermissionGranted() || await requestPermission() === "granted"
}
export async function deliverDesktopNotification(item: Pick<AppNotification, "title" | "body">) {
  if (!native || !await isPermissionGranted()) throw new Error("Desktop notification permission is unavailable")
  await invoke("plugin:notification|notify", { options: { title: item.title, body: item.body } })
}
export function useNotifications(profiles: Profile[], checks: Record<string, LimitCheckState>, version: string | null, prefs: AppPreferences, ready: boolean) {
  const [state, setState] = useState(readNotifications)
  const current = useRef(state)
  const [deliveryError, setDeliveryError] = useState("")
  useEffect(() => {
    if (!ready) return
    let next = current.current
    const added: AppNotification[] = []
    for (const profile of profiles) {
      const check = checks[profile.id]
      if (profile.authMode.toLowerCase() !== "chatgpt" || !check) continue
      const authentication = observeSignIn(next, profile.id, profile.name, check, prefs, Date.now())
      next = authentication.state; added.push(...authentication.added)
      if (!check.data || check.loading || check.error) continue
      const result = observeAccount(next, profile.id, profile.name, check.data, prefs, Date.now())
      next = result.state; added.push(...result.added)
    }
    if (version && !next.receipts.includes(`update:${version}`)) {
      const item: AppNotification = { id: `update:${version}`, title: `Multi Codex ${version} is available`, body: "Open the Update button to review and install it when you’re ready.", kind: "update", createdAt: Date.now(), read: false }
      next = { ...next, receipts: [...next.receipts, item.id].slice(-2048), items: prefs.notifications && prefs.updates ? [item, ...next.items].slice(0, 100) : next.items }
      if (prefs.notifications && prefs.updates) added.push(item)
    }
    const ids = new Set(profiles.map(p => p.id))
    // Account removal discards its retained usage and notification details.
    if ([...Object.keys(next.accounts), ...Object.keys(next.authentication)].some(id => !ids.has(id))) next = { ...next, authentication: Object.fromEntries(Object.entries(next.authentication).filter(([id]) => ids.has(id))), accounts: Object.fromEntries(Object.entries(next.accounts).filter(([id]) => ids.has(id))), items: next.items.filter(item => !item.accountId || ids.has(item.accountId)) }
    if (next === current.current) return
    current.current = next
    setState(next)
    try { localStorage.setItem(notificationKey, JSON.stringify(next)) } catch { setDeliveryError("Notification history could not be saved. Check available storage.") }
    if (prefs.desktopNotifications) for (const item of added) void deliverDesktopNotification(item).catch(() => setDeliveryError("Desktop alerts are unavailable. Your alerts are still saved here."))
  }, [profiles, checks, version, prefs, ready])
  function updateItems(items: AppNotification[]) {
    const next = { ...current.current, items }
    current.current = next; setState(next)
    try { localStorage.setItem(notificationKey, JSON.stringify(next)) } catch { setDeliveryError("Notification history could not be saved. Check available storage.") }
  }
  return { items: state.items, deliveryError, markRead: () => updateItems(current.current.items.map(item => ({ ...item, read: true }))), clear: () => updateItems([]) }
}
