import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { MoreHorizontalIcon, DashboardSquare01Icon, Logout01Icon, Orbit01Icon, PlusSignIcon, RefreshIcon, Settings01Icon } from "@hugeicons/core-free-icons"
import { HugeiconsIcon } from "@hugeicons/react"
import { useEffect, useRef, useState } from "react"
import "./MenuBar.css"

type Account = { id: string; name: string; icon: string | null }
type MenuContents = { accounts: Account[]; codex_available: boolean; targets: [string, string][]; probe: boolean }
const controls = [
  { action: "open", label: "Open Multi Codex", icon: DashboardSquare01Icon },
  { action: "add", label: "Add account", icon: PlusSignIcon },
  { action: "hud-toggle", label: "Show / hide Orbit", icon: Orbit01Icon },
  { action: "hud-settings", label: "Orbit settings", icon: Settings01Icon },
  { action: "refresh", label: "Refresh all usage", icon: RefreshIcon },
  { action: "quit", label: "Quit Multi Codex", icon: Logout01Icon },
]

export default function MenuBar() {
  const [contents, setContents] = useState<MenuContents | null>(null)
  const [expanded, setExpanded] = useState<string | null>(null)
  const accounts = contents?.accounts
  const targets = contents?.targets ?? []
  const [error, setError] = useState<string | null>(null)
  const panel = useRef<HTMLElement>(null)

  useEffect(() => {
    let active = true
    const refresh = () => {
      setError(null)
      setExpanded(null)
      panel.current?.focus()
      void invoke<MenuContents>("menu_bar_accounts").then(next => {
        if (active) setContents(next)
      }).catch(() => {
        if (active) setError("Could not load accounts. Reopen the menu to retry.")
      })
    }
    const stop = listen("menu-bar-open", refresh)
    refresh()
    function dismiss(event: KeyboardEvent) {
      if (event.key === "Escape") void invoke("menu_bar_action", { action: "dismiss" })
    }
    window.addEventListener("keydown", dismiss)
    return () => {
      active = false
      void stop.then(unlisten => unlisten()).catch(console.error)
      window.removeEventListener("keydown", dismiss)
    }
  }, [])

  useEffect(() => {
    if (!contents?.probe) return
    let frame = 0
    function reportButtons() {
      cancelAnimationFrame(frame)
      frame = requestAnimationFrame(() => {
        const buttons: Record<string, [number, number]> = {}
        panel.current?.querySelectorAll("button:not(:disabled)").forEach(button => {
          const rect = button.getBoundingClientRect()
          const x = rect.left + rect.width / 2
          const y = rect.top + rect.height / 2
          const label = button.getAttribute("aria-label") ?? button.textContent?.trim()
          if (label && button.contains(document.elementFromPoint(x, y))) {
            buttons[label] = [x, y]
          }
        })
        void invoke("menu_bar_probe_buttons", { buttons }).catch(console.error)
      })
    }
    reportButtons()
    window.addEventListener("resize", reportButtons)
    window.addEventListener("focus", reportButtons)
    return () => {
      window.removeEventListener("resize", reportButtons)
      window.removeEventListener("focus", reportButtons)
      cancelAnimationFrame(frame)
    }
  }, [contents, expanded])

  function toggleTargets(id: string) {
    const next = expanded === id ? null : id
    setExpanded(next)
    void invoke("menu_bar_expand", { rows: next ? targets.length : 0 }).catch(console.error)
  }

  function act(action: string, id?: string, target?: string) {
    void invoke("menu_bar_action", { action, id, target }).catch(() => setError("Could not complete that action. Try again."))
  }

  return <main className="menu-bar-panel" ref={panel} tabIndex={-1} aria-label="Multi Codex menu">
    <div className="menu-bar-heading"><h1>Accounts</h1></div>
    <div className="menu-bar-accounts">
      {error && <p role="alert">{error}</p>}
      {!error && !accounts && <p role="status">Loading accounts…</p>}
      {!error && accounts?.length === 0 && <p>No saved accounts</p>}
      {accounts?.map(account => <div key={account.id}>
        <div className="menu-bar-account-row">
          <button className="menu-bar-account" disabled={!contents?.codex_available}
            onClick={() => act("launch", account.id)} aria-label={`Open Codex as ${account.name}`}
            title={contents?.codex_available ? undefined : "Codex desktop app is not installed"}>
            <span className="menu-bar-avatar" aria-hidden="true">
              {account.icon ? <img src={`data:image/png;base64,${account.icon}`} alt="" /> : <span>{Array.from(account.name.trim())[0]?.toUpperCase()}</span>}
            </span>
            <span className="menu-bar-account-name">{account.name}</span>
          </button>
          {targets.length > 0 && <button className="menu-bar-more" aria-label={`More ways to open ${account.name}`}
            aria-expanded={expanded === account.id} onClick={() => toggleTargets(account.id)}>
            <HugeiconsIcon icon={MoreHorizontalIcon} size={16} aria-hidden="true" />
          </button>}
        </div>
        {expanded === account.id && <div className="menu-bar-targets">
          {targets.map(([target, label]) => <button key={target} onClick={() => act("launch-target", account.id, target)}>{label}</button>)}
        </div>}
      </div>)}
    </div>
    <div className="menu-bar-controls" aria-label="Multi Codex controls">
      {controls.map(({ action, label, icon }) => <button key={action} onClick={() => act(action)}>
        <HugeiconsIcon icon={icon} size={16} aria-hidden="true" />{label}
      </button>)}
    </div>
  </main>
}
