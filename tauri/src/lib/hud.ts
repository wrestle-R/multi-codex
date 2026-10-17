import { invoke } from "@tauri-apps/api/core"
import type { BotAvatarType } from "bot-avatars"

export interface AvatarIdentity { kind: BotAvatarType; color: string; seed: number }
export interface HudSettings {
  enabled: boolean; size: number; opacity: number; monitor: string | null; x: number | null; y: number | null
  onlyOpen: boolean; animation: boolean; reducedMotion: boolean; sound: boolean
}
export interface HudSession {
  recent?: boolean; recency?: number
  id: string; title: string; cwd: string; status: string; attention: string | null; epoch: number
  eventKey: string; needsAttention: boolean
  client?: "desktop" | "cli" | "vscode"; processId?: number
  tokens: { total: number | null; input: number | null; output: number | null; contextWindow: number | null } | null
}
export interface RateWindow { usedPercent: number | null; resetsAt: number | null; windowDurationMins: number | null }
export interface HudAccount { id: string; name: string; avatar: AvatarIdentity; sessions: HudSession[]; monitoringMessage?: string | null; limits: { primary: RateWindow; secondary: RateWindow } | null }
export interface HudView { settings: HudSettings; accounts: HudAccount[]; panelAbove?: boolean; platformError?: string | null }
export const defaultHudSettings: HudSettings = { enabled: true, size: 56, opacity: 0.95, monitor: null, x: null, y: null, onlyOpen: false, animation: true, reducedMotion: false, sound: false }
export const nativeHud = Boolean(window.__TAURI_INTERNALS__)
export function hudCall<T>(method: string, params?: Record<string, unknown>): Promise<T> {
  if (nativeHud) return invoke<T>(method, params)
  if (method === "get_update_channel") return Promise.resolve((localStorage.getItem("multi-codex-update-channel") === "all" ? "all" : "stable") as T)
  if (method === "set_update_channel") localStorage.setItem("multi-codex-update-channel", params?.channel === "all" ? "all" : "stable")
  if (method === "hud_monitors") return Promise.resolve(["Preview display"] as T)
  if (method === "hud_snapshot") return Promise.resolve({ settings: defaultHudSettings, accounts: [] } as T)
  return Promise.resolve(undefined as T)
}
export function orderedSessions(sessions: HudSession[]) {
  return [...sessions].sort((a, b) => Number(Boolean(b.recent)) - Number(Boolean(a.recent)) || (a.recent && b.recent ? (b.recency ?? 0) - (a.recency ?? 0) || a.id.localeCompare(b.id) : 0) || Number(b.needsAttention) - Number(a.needsAttention) || Number(b.status === "active") - Number(a.status === "active") || a.id.localeCompare(b.id))
}
export function sessionSlots(sessions: HudSession[]): Array<HudSession | { overflow: number }> {
  const ordered = orderedSessions(sessions)
  return ordered.length > 4 ? [...ordered.slice(0, 3), { overflow: ordered.length - 3 }] : ordered
}
export function attentionLabel(session: HudSession) {
  if (session.needsAttention) return ({ question: "Question waiting", approval: "Approval waiting", mayNeedYou: "May need you", error: "Chat error" } as Record<string, string>)[session.attention ?? ""] ?? "Needs attention"
  return session.status === "active" ? "Working" : session.recent ? "Recent chat" : "Open"
}
export function attentionIcon(session: HudSession) {
  return session.needsAttention ? session.attention === "question" ? "?" : session.attention === "approval" ? "✓" : "!" : session.status === "active" ? "•" : "◦"
}
const kinds: BotAvatarType[] = ["clover", "flower", "ghost", "square", "cloud", "cat", "pebble", "blob", "star", "drop", "circle", "pill", "alien", "droid", "mech", "hexagon", "triangle", "puddle"]
const colors = ["#60c8df", "#73d8a6", "#c6b9e8", "#de8abc", "#e2b769", "#90acf1"]
const identities = new Map<string, Promise<AvatarIdentity>>()
export function accountAvatar(id: string): Promise<AvatarIdentity> {
  let identity = identities.get(id)
  if (!identity) {
    identity = crypto.subtle.digest("SHA-256", new TextEncoder().encode(id)).then(bytes => {
      const hash = new Uint8Array(bytes)
      return { kind: kinds[hash[0] % kinds.length], color: colors[hash[1] % colors.length], seed: new DataView(bytes).getUint32(2, true) }
    })
    identities.set(id, identity)
  }
  return identity
}
