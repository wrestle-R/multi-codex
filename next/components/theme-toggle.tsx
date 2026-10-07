"use client"

import { useEffect, useRef, useState } from "react"

type Theme = "light" | "dark"
const storageKey = "multi-codex-site-theme"

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>("light")
  const explicit = useRef(false)

  useEffect(() => {
    const system = window.matchMedia("(prefers-color-scheme: dark)")
    function apply(mode: Theme) {
      document.documentElement.dataset.theme = mode
      setTheme(mode)
    }
    function syncPreference() {
      let saved: string | null = null
      try { saved = localStorage.getItem(storageKey) } catch { /* Private browsing can block storage. */ }
      explicit.current = saved === "light" || saved === "dark"
      apply(saved === "light" || saved === "dark" ? saved : system.matches ? "dark" : "light")
    }
    function systemChanged(event: MediaQueryListEvent) {
      if (!explicit.current) apply(event.matches ? "dark" : "light")
    }
    function storageChanged(event: StorageEvent) {
      if (event.key === storageKey || event.key === null) syncPreference()
    }
    syncPreference()
    system.addEventListener("change", systemChanged)
    window.addEventListener("storage", storageChanged)
    return () => {
      system.removeEventListener("change", systemChanged)
      window.removeEventListener("storage", storageChanged)
    }
  }, [])

  function toggle() {
    const next = document.documentElement.dataset.theme === "dark" ? "light" : "dark"
    explicit.current = true
    document.documentElement.dataset.theme = next
    setTheme(next)
    try { localStorage.setItem(storageKey, next) } catch { /* The toggle still works for this visit. */ }
  }

  return <button type="button" className="theme-toggle" onClick={toggle} aria-label="Toggle color theme" aria-pressed={theme === "dark"} title={`Use ${theme === "dark" ? "light" : "dark"} theme`}>
    <svg className="theme-moon" aria-hidden="true" viewBox="0 0 24 24"><path d="M20.5 13A8.5 8.5 0 0 1 11 3.5 8.5 8.5 0 1 0 20.5 13Z" /></svg>
    <svg className="theme-sun" aria-hidden="true" viewBox="0 0 24 24"><circle cx="12" cy="12" r="4" /><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5" /></svg>
  </button>
}
