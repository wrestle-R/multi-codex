import { useEffect, useRef, useState } from "react"
import { listen } from "@tauri-apps/api/event"
import { AccountAvatar } from "./components/account-avatar"
import { attentionIcon, attentionLabel, defaultHudSettings, hudCall, nativeHud, orderedSessions, sessionSlots, type HudAccount, type HudSession, type HudView, type RateWindow } from "./lib/hud"
import { initialColorTheme } from "./lib/themes"
import { readLimitsCache } from "./lib/limits-cache"
import "./App.css"
import "./Hud.css"

function Usage({ account }: { account: HudAccount }) {
  const cached = readLimitsCache()[account.id]?.data
  const windows = account.limits ? [account.limits.primary, account.limits.secondary].filter(w => w.usedPercent != null).map((window: RateWindow) => ({ label: window.windowDurationMins === 300 ? "5 hours" : window.windowDurationMins === 10080 ? "Weekly" : "Usage", remaining: Math.max(0, 100 - (window.usedPercent ?? 0)), reset: window.resetsAt }))
    : cached ? [["5 hours", cached.fiveHour], ["Weekly", cached.weekly], ["Monthly", cached.monthly]].flatMap(([label, window]) => typeof window === "object" && window ? [{ label: String(label), remaining: window.remainingPercent, reset: window.resetsAt }] : []) : []
  return <div className="hud-usage">{windows.length ? windows.map((window, i) => <div key={i}><div><span>{window.label}</span><strong>{Math.round(window.remaining)}% left</strong></div><progress max={100} value={window.remaining} /><small>{window.reset ? `Resets ${new Date(window.reset * 1000).toLocaleString()}` : "Reset unavailable"}</small></div>) : <p>Limits have not been reported yet.</p>}</div>
}
function chime() {
  try {
    const audio = new AudioContext(); const tone = audio.createOscillator(); const gain = audio.createGain()
    tone.frequency.value = 660; gain.gain.setValueAtTime(0.025, audio.currentTime); gain.gain.exponentialRampToValueAtTime(0.0001, audio.currentTime + 0.18)
    tone.connect(gain); gain.connect(audio.destination); tone.start(); tone.stop(audio.currentTime + 0.2); tone.onended = () => { void audio.close() }
  } catch { /* A muted system still retains the visual reminder. */ }
}
export default function Hud() {
  const [view, setView] = useState<HudView>({ settings: defaultHudSettings, accounts: [] })
  const [hovered, setHovered] = useState<string | null>(null)
  const [popup, setPopup] = useState<{ account: HudAccount; session: HudSession } | null>(null)
  const [error, setError] = useState("")
  const [refreshVersion, setRefreshVersion] = useState(0)
  const seen = useRef(new Set<string>())
  const popupTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  useEffect(() => {
    document.documentElement.classList.add("hud-document")
    const theme = () => {
      const mode = localStorage.getItem("multi-codex-theme") ?? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
      document.documentElement.classList.toggle("dark", mode === "dark"); document.documentElement.dataset.theme = mode; document.documentElement.dataset.palette = initialColorTheme()
    }
    theme(); window.addEventListener("storage", theme)
    let active = true; let pending = false; let unlisten: (() => void) | undefined
    const refresh = async () => {
      if (pending) return; pending = true
      try {
        const next = await hudCall<HudView>("hud_snapshot")
        if (!active) return
        setView(next)
        const attention = next.accounts.flatMap(account => orderedSessions(account.sessions).filter(s => s.needsAttention && !seen.current.has(s.eventKey)).map(session => ({ account, session })))
        for (const event of attention) seen.current.add(event.session.eventKey)
        if (attention.length) {
          // Receipts prevent redisplaying a popup after a renderer restart;
          // these are separate from acknowledging the persistent badge.
          const notified = JSON.parse(sessionStorage.getItem("hud-notified") ?? "[]") as string[]
          const newEvent = attention.find(event => !notified.includes(event.session.eventKey))
          sessionStorage.setItem("hud-notified", JSON.stringify([...notified, ...attention.map(a => a.session.eventKey)].slice(-4096)))
          if (newEvent) { setPopup(newEvent); if (next.settings.sound) chime(); clearTimeout(popupTimer.current); popupTimer.current = setTimeout(() => setPopup(null), 2500) }
        }
      } catch (error) { if (active) setError(String(error)) }
      finally { pending = false }
    }
    void refresh()
    if (nativeHud) void listen("hud-refresh", refresh).then(off => { if (active) unlisten = off; else off() })
    const timer = setInterval(refresh, 1500)
    return () => { active = false; clearInterval(timer); clearTimeout(popupTimer.current); unlisten?.(); window.removeEventListener("storage", theme) }
  }, [refreshVersion])
  useEffect(() => { void hudCall("hud_set_expanded", { expanded: Boolean(hovered || popup || error) }) }, [hovered, popup, error])
  const open = (account: HudAccount, session?: HudSession) => {
    void hudCall("hud_open_session", { accountId: account.id, threadId: session?.id ?? null, eventKey: session?.needsAttention ? session.eventKey : null }).then(() => { setPopup(null); setError(""); setRefreshVersion(version => version + 1) }).catch(error => setError(String(error)))
  }
  const accounts = view.accounts.filter(account => !view.settings.onlyOpen || account.sessions.length > 0)
  const account = view.accounts.find(a => a.id === hovered)
  const reduced = view.settings.reducedMotion || matchMedia("(prefers-reduced-motion: reduce)").matches
  return <main className="hud-shell" style={{ opacity: view.settings.opacity, "--hud-slot": `${view.settings.size}px` } as React.CSSProperties} onMouseLeave={() => setHovered(null)} onKeyDown={event => { if (event.key === "Escape") { setHovered(null); setError("") } }}>
    <div className="hud-strip" aria-label="Codex Desktop accounts">
      <button className="hud-drag" aria-label="Drag HUD" onPointerDown={event => { if (event.button === 0) void hudCall("hud_drag") }}>⠿</button>
      {accounts.map(account => {
        const sessions = orderedSessions(account.sessions)
        return <div className="hud-account" key={account.id} onMouseEnter={() => setHovered(account.id)} onFocus={() => setHovered(account.id)}>
          {sessions.length < 2 ? <button className="hud-avatar-button" aria-label={`${account.name}: ${sessions[0] ? attentionLabel(sessions[0]) : "No open chats"}`} title={sessions[0]?.title ?? account.name} onClick={() => open(account, sessions[0])}>
            <AccountAvatar id={account.id} identity={account.avatar} size={view.settings.size} working={sessions[0]?.status === "active"} animate={view.settings.animation && !reduced} />
            {sessions.length > 0 && <span className={`hud-presence ${sessions[0].needsAttention ? "needs-attention" : sessions[0].status}`} aria-hidden="true">{attentionIcon(sessions[0])}</span>}
          </button> : <div className={`hud-dots ${sessions.length === 2 ? "two" : "grid"}`}>{sessionSlots(sessions).map((slot, index) => "overflow" in slot ? <button key="overflow" className="hud-dot hud-overflow" aria-label={`${slot.overflow} more ${account.name} chats`} onClick={() => setHovered(account.id)}>+{slot.overflow}</button> : <button className={`hud-dot ${slot.needsAttention ? "needs-attention" : slot.status}`} key={slot.id} style={{ "--account-color": account.avatar.color } as React.CSSProperties} title={slot.title} aria-label={`${account.name}: ${slot.title}, ${attentionLabel(slot)}`} onClick={() => open(account, slot)}><span aria-hidden="true">{slot.needsAttention ? attentionIcon(slot) : index + 1}</span></button>)}</div>}
        </div>
      })}
      {!accounts.length && <span className="hud-empty">Codex</span>}
      <button className="hud-hide" aria-label="Hide HUD" onClick={() => { void hudCall("hud_toggle") }}>−</button>
    </div>
    {account && <section className="hud-panel" aria-label={`${account.name} usage and sessions`}>
      <header><AccountAvatar id={account.id} identity={account.avatar} size={36} /><div><strong>{account.name}</strong><small>{account.sessions.length} open {account.sessions.length === 1 ? "chat" : "chats"}</small></div></header>
      <Usage account={account} />
      <div className="hud-session-list">{orderedSessions(account.sessions).map(session => <button key={session.id} className="hud-session" onClick={() => open(account, session)}><span className={`hud-session-status ${session.needsAttention ? "needs-attention" : session.status}`}>{attentionIcon(session)}</span><span><strong>{session.title || "Codex chat"}</strong><small>{attentionLabel(session)}</small>{session.tokens && <small>{session.tokens.total?.toLocaleString() ?? "—"} tokens{session.tokens.contextWindow ? ` · ${session.tokens.input?.toLocaleString() ?? "—"}/${session.tokens.contextWindow.toLocaleString()} context` : ""}</small>}</span><span aria-hidden="true">↗</span></button>)}</div>
    </section>}
    {popup && !account && <button className="hud-popup" aria-label="Open chat needing attention" onClick={() => open(popup.account, popup.session)}><AccountAvatar id={popup.account.id} identity={popup.account.avatar} size={42} /><span aria-hidden="true">{attentionIcon(popup.session)}</span></button>}
    {error && <p className="hud-error" role="alert">{error}<button aria-label="Dismiss navigation error" onClick={() => setError("")}>×</button></p>}
  </main>
}
