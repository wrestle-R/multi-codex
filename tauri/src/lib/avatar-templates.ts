import { useEffect, useSyncExternalStore } from "react"
import type { AvatarIdentity } from "./hud"

export const avatarTemplates = [
  { name: "Sky", kind: "cloud", color: "#60c8df", seed: 120 },
  { name: "Mint", kind: "cat", color: "#73d8a6", seed: 241 },
  { name: "Lilac", kind: "ghost", color: "#c6b9e8", seed: 362 },
  { name: "Rose", kind: "flower", color: "#de8abc", seed: 483 },
  { name: "Honey", kind: "star", color: "#e2b769", seed: 604 },
  { name: "Blue", kind: "droid", color: "#90acf1", seed: 725 },
] as const satisfies readonly (AvatarIdentity & { name: string })[]
export function templateFor(identity: AvatarIdentity) {
  const index = avatarTemplates.findIndex(template => template.color.toLowerCase() === identity.color.toLowerCase())
  return index >= 0 ? index : identity.seed % avatarTemplates.length
}
export function balanceTemplates(ids: string[], existing: Record<string, number>): Record<string, number> {
  const next = Object.fromEntries(Object.entries(existing).filter(([, index]) => Number.isInteger(index) && index >= 0 && index < avatarTemplates.length))
  const unique = [...new Set(ids)].sort()
  const counts = avatarTemplates.map((_, index) => unique.filter(id => next[id] === index).length)
  for (const id of unique) {
    if (next[id] !== undefined) continue
    const index = counts.indexOf(Math.min(...counts))
    next[id] = index; counts[index]++
  }
  return next
}
const key = `multi-codex-avatar-templates:v1:${window.__TAURI_INTERNALS__ && new URLSearchParams(location.search).get("demo") !== "1" ? "native" : "preview"}`
let saved: Record<string, number> | undefined
function snapshot() {
  if (saved) return saved
  try { const value = JSON.parse(localStorage.getItem(key) ?? "{}"); saved = value && typeof value === "object" && !Array.isArray(value) ? balanceTemplates([], value) : {} } catch { saved = {} }
  return saved!
}
function subscribe(callback: () => void) {
  const update = (event: Event) => { if (event instanceof StorageEvent) { if (event.key !== key && event.key !== null) return; saved = undefined } callback() }
  window.addEventListener("storage", update); window.addEventListener("avatar-templates", update)
  return () => { window.removeEventListener("storage", update); window.removeEventListener("avatar-templates", update) }
}
export function useAvatarTemplate(id: string) { return useSyncExternalStore(subscribe, snapshot)[id] }
export function useBalancedAvatars(ids: string[]) {
  const signature = JSON.stringify([...ids].sort())
  useEffect(() => {
    const previous = snapshot(), next = balanceTemplates(JSON.parse(signature), previous)
    if (JSON.stringify(previous) === JSON.stringify(next)) return
    saved = next
    try { localStorage.setItem(key, JSON.stringify(next)) } catch { /* Keep the current session consistent even if storage is full. */ }
    window.dispatchEvent(new Event("avatar-templates"))
    saved = next
  }, [signature])
}
